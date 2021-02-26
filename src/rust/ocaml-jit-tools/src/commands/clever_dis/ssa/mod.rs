use anyhow::{bail, ensure, Result};

use ocaml_jit_shared::{ArithOp, Instruction, Primitive};

use crate::commands::clever_dis::data::{Block, BlockExit, Closure};
use crate::commands::clever_dis::ssa::data::{
    BinaryFloatOp, ModifySSAVars, SSABlock, SSAClosure, SSAExit, SSAExpr, SSAStatement,
    SSASubstitutionTarget, SSAVar, UnaryFloatOp, UnaryOp,
};
use crate::commands::clever_dis::ssa::stack_state::SSAStackState;
use std::collections::{HashMap, HashSet};

mod stack_state;

#[cfg(test)]
mod test_block_translation;

#[cfg(test)]
mod test_closure_translation;

pub mod data;

struct Vars {
    statements: Vec<SSAStatement>,
    num_assignments: usize,
    block_num: usize,
}

impl Vars {
    fn new(block_num: usize) -> Vars {
        Vars {
            statements: Vec::new(),
            num_assignments: 0,
            block_num,
        }
    }

    fn add_statement(&mut self, statement: SSAStatement) {
        self.statements.push(statement);
    }

    fn add_assignment(&mut self, expr: SSAExpr) -> SSAVar {
        let assignment_num = self.num_assignments;
        self.num_assignments += 1;

        self.add_statement(SSAStatement::Assign(self.block_num, assignment_num, expr));
        SSAVar::Computed(self.block_num, assignment_num)
    }
}

fn get_blocks(closure: &Closure) -> Result<Vec<SSABlock>> {
    let mut blocks = vec![];
    for (block_num, b) in closure.blocks.iter().enumerate() {
        let is_entry_block = block_num == 0 && !closure.is_root;
        let is_trap_handler = closure.trap_handlers.contains(&block_num);
        blocks.push(translate_block(
            b,
            block_num,
            is_entry_block,
            is_trap_handler,
        )?);
    }
    Ok(blocks)
}

pub fn translate_closure(closure: &Closure, use_relocations: bool) -> Result<SSAClosure> {
    let mut blocks = get_blocks(closure)?;
    if use_relocations {
        relocate_blocks(&mut blocks)?;
    }

    Ok(SSAClosure { blocks })
}

// Initial block translation
// Convert a list of bytecode instructions using the stack into a list of SSA instructions
// but only considering each basic block locally

pub fn translate_block(
    block: &Block,
    block_num: usize,
    is_entry_block: bool,
    is_trap_handler: bool,
) -> Result<SSABlock> {
    ensure!(!block.instructions.is_empty());
    let last_instr_idx = block.instructions.len() - 1;

    let mut vars_d = Vars::new(block_num);
    let mut state_d = SSAStackState::new();
    let vars = &mut vars_d;
    let state = &mut state_d;

    if is_trap_handler {
        state.set_accu(SSAVar::TrapAcc);
        state.pop(4);
    }

    let has_grab = if is_entry_block {
        state.set_accu(SSAVar::Junk);
        if let Instruction::Grab(nargs) = &block.instructions[0] {
            let nargs = *nargs as usize;
            for arg in (0..=nargs).rev() {
                state.push(SSAVar::Arg(arg));
            }

            vars.add_statement(SSAStatement::Grab(nargs));

            true
        } else {
            state.push(SSAVar::Arg(0));
            false
        }
    } else {
        false
    };
    let first_instr_idx = if has_grab { 1 } else { 0 };

    for instr in &block.instructions[first_instr_idx..last_instr_idx] {
        process_body_instruction(state, vars, instr)?;
    }

    let last_instruction = block.instructions.last().unwrap();
    let exit = process_final_instruction(state, vars, last_instruction, &block.exit)?;

    Ok(SSABlock {
        statements: vars_d.statements,
        exit,
        final_state: state_d,
    })
}

fn process_body_instruction(
    state: &mut SSAStackState,
    vars: &mut Vars,
    instr: &Instruction<usize>,
) -> Result<()> {
    match instr {
        Instruction::ApplyTerm(_, _)
        | Instruction::Apply(_)
        | Instruction::Return(_)
        | Instruction::Branch(_)
        | Instruction::BranchIf(_)
        | Instruction::BranchIfNot(_)
        | Instruction::BranchCmp(_, _, _)
        | Instruction::Raise(_)
        | Instruction::Switch(_, _)
        | Instruction::PushTrap(_)
        | Instruction::Stop => {
            bail!("{:?} should be last call in a block!", instr);
        }
        Instruction::Restart => {
            bail!("Restarts should not appear in blocks");
        }
        Instruction::LabelDef(_) => {}
        Instruction::Acc(n) => {
            let v = state.pick(*n as usize);
            state.set_accu(v);
        }
        Instruction::EnvAcc(n) => {
            state.set_accu(SSAVar::Env(*n as usize));
        }
        Instruction::Push => {
            let v = state.accu();
            state.push(v);
        }
        Instruction::Pop(n) => {
            state.pop(*n as usize);
        }
        Instruction::Assign(n) => {
            let v = state.accu();
            state.assign(*n as usize, v);
        }
        Instruction::PushRetAddr(_) => {
            state.push(SSAVar::Special);
            state.push(SSAVar::Special);
            state.push(SSAVar::Special);
        }
        Instruction::Apply1 => {
            let v = vars.add_assignment(SSAExpr::Apply(state.accu(), vec![state.pick(0)]));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::Apply2 => {
            let v = vars.add_assignment(SSAExpr::Apply(
                state.accu(),
                vec![state.pick(0), state.pick(1)],
            ));
            state.set_accu(v);
            state.pop(2);
        }
        Instruction::Apply3 => {
            let v = vars.add_assignment(SSAExpr::Apply(
                state.accu(),
                vec![state.pick(0), state.pick(1), state.pick(2)],
            ));
            state.set_accu(v);
            state.pop(3);
        }
        Instruction::Grab(_) => bail!("Grabs should not appear in the body of blocks"),
        Instruction::Closure(loc, nvars) => {
            let nvars = *nvars as usize;
            if nvars > 0 {
                let accu = state.accu();
                state.push(accu);
            }

            let v = vars.add_assignment(SSAExpr::Closure {
                code: *loc,
                vars: (0..nvars).map(|i| state.pick(i)).collect(),
            });
            state.set_accu(v);

            state.pop(nvars);
        }
        Instruction::ClosureRec(locs, nvars) => {
            let nvars = *nvars as usize;
            if nvars > 0 {
                let accu = state.accu();
                state.push(accu);
            }

            let v = vars.add_assignment(SSAExpr::ClosureRec {
                codes: locs.clone(),
                vars: (0..nvars).map(|i| state.pick(i)).collect(),
            });

            state.pop(nvars);
            state.set_accu(v);
            state.push(v);

            for i in 1..locs.len() {
                state.push(vars.add_assignment(SSAExpr::ClosureRecInfix(v, i)));
            }
        }
        Instruction::OffsetClosure(i) => {
            state.set_accu(SSAVar::OffsetClosure(*i as isize));
        }
        Instruction::GetGlobal(n) => {
            state.set_accu(vars.add_assignment(SSAExpr::GetGlobal(*n as usize)));
        }
        Instruction::SetGlobal(n) => {
            vars.add_statement(SSAStatement::SetGlobal(*n as usize, state.accu()));
            state.set_accu(SSAVar::Unit);
        }
        Instruction::Const(v) => {
            state.set_accu(SSAVar::Const(*v));
        }
        Instruction::MakeBlock(size, tag) => {
            let size = *size as usize;
            let tag = *tag;
            if size == 0 {
                state.set_accu(SSAVar::Atom(tag));
            } else {
                let accu = state.accu();
                state.push(accu);

                let v = vars.add_assignment(SSAExpr::MakeBlock {
                    tag,
                    vars: (0..size).map(|i| state.pick(i)).collect(),
                });
                state.set_accu(v);
                state.pop(size);
            }
        }
        Instruction::MakeFloatBlock(size) => {
            let size = *size as usize;
            let accu = state.accu();
            state.push(accu);
            let v = vars.add_assignment(SSAExpr::MakeFloatBlock(
                (0..size).map(|i| state.pick(i)).collect(),
            ));
            state.set_accu(v);
            state.pop(size);
        }
        Instruction::GetField(n) => {
            let v = vars.add_assignment(SSAExpr::GetField(state.accu(), SSAVar::Const(*n as i32)));
            state.set_accu(v);
        }
        Instruction::SetField(n) => {
            vars.add_statement(SSAStatement::SetField(
                state.accu(),
                SSAVar::Const(*n as i32),
                state.pick(0),
            ));
            state.pop(1);
            state.set_accu(SSAVar::Unit);
        }
        Instruction::GetFloatField(n) => {
            let v = vars.add_assignment(SSAExpr::GetFloatField(state.accu(), *n as usize));
            state.set_accu(v);
        }
        Instruction::SetFloatField(n) => {
            vars.add_statement(SSAStatement::SetFloatField(
                state.accu(),
                *n as usize,
                state.pick(0),
            ));
            state.pop(1);
            state.set_accu(SSAVar::Unit);
        }
        Instruction::VecTLength => {
            let v = vars.add_assignment(SSAExpr::GetVecTLength(state.accu()));
            state.set_accu(v);
        }
        Instruction::GetVecTItem => {
            let v = vars.add_assignment(SSAExpr::GetField(state.accu(), state.pick(0)));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::SetVecTItem => {
            vars.add_statement(SSAStatement::SetField(
                state.accu(),
                state.pick(0),
                state.pick(1),
            ));
            state.pop(2);
            state.set_accu(SSAVar::Unit);
        }
        Instruction::GetBytesChar => {
            let v = vars.add_assignment(SSAExpr::GetBytesChar(state.accu(), state.pick(0)));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::SetBytesChar => {
            vars.add_statement(SSAStatement::SetBytesChar(
                state.accu(),
                state.pick(0),
                state.pick(1),
            ));
            state.pop(2);
            state.set_accu(SSAVar::Unit);
        }
        Instruction::BoolNot => {
            let v = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::BoolNot, state.accu()));
            state.set_accu(v);
        }
        Instruction::PopTrap => {
            vars.add_statement(SSAStatement::PopTrap);
            state.pop(4);
        }
        Instruction::CheckSignals => {
            vars.add_statement(SSAStatement::CheckSignals);
            state.set_accu(SSAVar::Junk);
        }
        Instruction::Prim(p) => match p {
            Primitive::NegFloat => unary_float(state, vars, UnaryFloatOp::Neg),
            Primitive::SqrtFloat => unary_float(state, vars, UnaryFloatOp::Sqrt),
            Primitive::AddFloat => binary_float(state, vars, BinaryFloatOp::Add),
            Primitive::SubFloat => binary_float(state, vars, BinaryFloatOp::Sub),
            Primitive::MulFloat => binary_float(state, vars, BinaryFloatOp::Mul),
            Primitive::DivFloat => binary_float(state, vars, BinaryFloatOp::Div),
        },
        Instruction::CCall1(id) => c_call(state, vars, 1, id),
        Instruction::CCall2(id) => c_call(state, vars, 2, id),
        Instruction::CCall3(id) => c_call(state, vars, 3, id),
        Instruction::CCall4(id) => c_call(state, vars, 4, id),
        Instruction::CCall5(id) => c_call(state, vars, 5, id),
        Instruction::CCallN(nargs, id) => c_call(state, vars, *nargs as usize, id),
        Instruction::ArithInt(op) => {
            let v = vars.add_assignment(SSAExpr::ArithInt(*op, state.accu(), state.pick(0)));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::NegInt => {
            let v = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::Neg, state.accu()));
            state.set_accu(v);
        }
        Instruction::IntCmp(comp) => {
            let v = vars.add_assignment(SSAExpr::IntCmp(*comp, state.accu(), state.pick(0)));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::OffsetInt(n) => {
            let v = vars.add_assignment(SSAExpr::ArithInt(
                ArithOp::Add,
                state.accu(),
                SSAVar::Const(*n),
            ));
            state.set_accu(v);
        }
        Instruction::OffsetRef(n) => {
            // Todo investigate whether special casing helps avoid a caml_modify
            let a = vars.add_assignment(SSAExpr::GetField(state.accu(), SSAVar::Const(0)));
            let b = vars.add_assignment(SSAExpr::ArithInt(ArithOp::Add, a, SSAVar::Const(*n)));
            vars.add_statement(SSAStatement::SetField(state.accu(), SSAVar::Const(0), b));
            state.set_accu(SSAVar::Unit);
        }
        Instruction::IsInt => {
            let v = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::IsInt, state.accu()));
            state.set_accu(v);
        }
        Instruction::GetMethod => {
            let v = vars.add_assignment(SSAExpr::GetMethod(state.pick(0), state.accu()));
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::SetupForPubMet(n) => {
            let accu = state.accu();
            state.push(accu);
            state.set_accu(SSAVar::Const(*n));
        }
        Instruction::GetDynMet => {
            let v = vars.add_assignment(SSAExpr::GetDynMet {
                tag: state.accu(),
                object: state.pick(0),
            });
            state.set_accu(v);
            state.pop(1);
        }
        Instruction::Break | Instruction::Event => (),
    }

    Ok(())
}

fn process_final_instruction(
    state: &mut SSAStackState,
    vars: &mut Vars,
    instr: &Instruction<usize>,
    exit: &BlockExit,
) -> Result<SSAExit> {
    let exit = match (instr, exit) {
        (Instruction::Stop, BlockExit::Stop) => SSAExit::Stop(state.accu()),
        (Instruction::Branch(n2), BlockExit::UnconditionalJump(n1)) => {
            ensure!(n1 == n2);
            SSAExit::Jump(*n1)
        }
        (Instruction::Return(count), BlockExit::Return) => {
            state.pop(*count as usize);
            SSAExit::Return(state.accu())
        }
        (Instruction::BranchIf(to1), BlockExit::ConditionalJump(ift, iff)) => {
            ensure!(to1 == ift);
            SSAExit::JumpIf {
                var: state.accu(),
                if_true: *ift,
                if_false: *iff,
            }
        }
        (Instruction::BranchIfNot(to1), BlockExit::ConditionalJump(iff, ift)) => {
            ensure!(to1 == iff);
            SSAExit::JumpIf {
                var: state.accu(),
                if_true: *ift,
                if_false: *iff,
            }
        }
        (Instruction::BranchCmp(comp, compare, ift1), BlockExit::ConditionalJump(ift2, iff)) => {
            ensure!(ift1 == ift2);
            let v = vars.add_assignment(SSAExpr::IntCmp(
                *comp,
                SSAVar::Const(*compare),
                state.accu(),
            ));
            SSAExit::JumpIf {
                var: v,
                if_true: *ift1,
                if_false: *iff,
            }
        }
        (Instruction::Raise(kind), BlockExit::Raise) => SSAExit::Raise(*kind, state.accu()),
        (Instruction::Apply(nvars), BlockExit::UnconditionalJump(retloc)) => {
            let nvars = *nvars as usize;
            let retloc = *retloc;
            ensure!(nvars > 3);

            let passed_vars = (0..nvars).map(|n| state.pick(n)).collect();
            state.pop(nvars);

            /*
            let retloc2 = match state.pick(0) {
                SSAVar::RetLoc(l) => l,
                o => bail!("Expected return location but got {}", o),
            };
            ensure!(retloc1 == retloc2);
            ensure!(state.pick(1) == SSAVar::RetEnv);
            ensure!(state.pick(2) == SSAVar::RetExtraArgs);
             */
            state.pop(3);

            let v = vars.add_assignment(SSAExpr::Apply(state.accu(), passed_vars));
            state.set_accu(v);
            SSAExit::Jump(retloc)
        }
        (Instruction::ApplyTerm(nargs, slotsize), BlockExit::TailCall) => {
            let nargs = *nargs as usize;
            let slotsize = *slotsize as usize;
            let vars = (0..nargs).map(|i| state.pick(i)).collect();
            state.pop(slotsize);
            SSAExit::TailApply(state.accu(), vars)
        }
        (
            Instruction::Switch(ints1, blocks1),
            BlockExit::Switch {
                ints: ints2,
                blocks: blocks2,
            },
        ) => {
            ensure!(ints1 == ints2);
            ensure!(blocks1 == blocks2);
            SSAExit::Switch {
                var: state.accu(),
                ints: ints1.clone(),
                blocks: blocks1.clone(),
            }
        }

        (
            Instruction::PushTrap(trap1),
            BlockExit::PushTrap {
                normal,
                trap: trap2,
            },
        ) => {
            ensure!(trap1 == trap2);

            for _ in 0..4 {
                state.push(SSAVar::Special);
            }

            SSAExit::PushTrap {
                normal: *normal,
                trap: *trap1,
            }
        }
        (i, BlockExit::UnconditionalJump(to)) => {
            process_body_instruction(state, vars, i)?;
            SSAExit::Jump(*to)
        }

        (i, e) => bail!("Invalid block exit: {:?} {:?}", i, e),
    };

    Ok(exit)
}

fn c_call(state: &mut SSAStackState, vars: &mut Vars, count: usize, primitive_id: &u32) {
    let accu = state.accu();
    state.push(accu);

    let v = vars.add_assignment(SSAExpr::CCall {
        primitive_id: *primitive_id as usize,
        vars: (0..count).map(|i| state.pick(i)).collect(),
    });
    state.set_accu(v);
    state.pop(count);
}

fn unary_float(state: &mut SSAStackState, vars: &mut Vars, op: UnaryFloatOp) {
    let v = vars.add_assignment(SSAExpr::UnaryFloat(op, state.accu()));
    state.set_accu(v);
}

fn binary_float(state: &mut SSAStackState, vars: &mut Vars, op: BinaryFloatOp) {
    let v = vars.add_assignment(SSAExpr::BinaryFloat(op, state.accu(), state.pick(0)));
    state.set_accu(v);
    state.pop(1);
}

// Relocation
// ==========
// Use the CFG to patch together variables between blocks handling branches and loops
// by inserting phi nodes
//
// The reverse post order is the order we want to process stack unifications in as it's broadly
// compatible with program flow. Where it isn't that's because the edge is a back edge due to a loop
//
// We return a mapping of original (effectively pre-order) block numbers to the reverse post order
//
// TODO: (maybe) investigate modifying original block numbering to be a reverse-post-order
//       would be cleaner but maybe not worth it at this point
//
// This is true for OCaml CFGs but possibly not true for all graphs
//
// We also use the fact we're searching to validate the following invariants:
// 1. Every path to the start of the block has the same stack stack size
// 2. Every normal return (Return/Stop i.e. not Raise) ends with a stack size of 0
// 3. At no point does the stack size dip below 0
// 4. Every block is actually reachable from block 0 (we get this after removing Restart)
//
// Err is returned on failure of any of these invariants. These represent a
// mistake in the SSA conversion code as they are true in general for the output of
// the OCaml compiler
//
// So validating the invariants is a good sanity check of being sensible with our approach to the stack
fn dfs_blocks(blocks: &[SSABlock]) -> Result<(Vec<usize>, Vec<usize>)> {
    fn dfs(
        blocks: &[SSABlock],
        sizes: &mut HashMap<usize, usize>,
        post_order: &mut Vec<usize>,
        block_num: usize,
        start_size: usize,
    ) -> Result<()> {
        let b = &blocks[block_num];

        sizes.insert(block_num, start_size);
        let new_size = start_size as isize + b.final_state.delta();
        ensure!(new_size >= 0); // Invariant 3
        let new_size = new_size as usize;

        match b.exit {
            SSAExit::Stop(_) => {
                ensure!(new_size == 0); // invariant 2
            }
            SSAExit::Return(_) => {
                ensure!(new_size == 0); // invariant 2
            }
            _ => {
                for other_block in b.exit.referenced_blocks() {
                    match sizes.get(&other_block) {
                        Some(&existing_size) => {
                            ensure!(new_size == existing_size); // invariant 1
                        }
                        None => {
                            dfs(blocks, sizes, post_order, other_block, new_size)?;
                        }
                    }
                }
            }
        }
        post_order.push(block_num);

        Ok(())
    }

    let mut sizes = HashMap::new();
    let mut post_order = vec![];

    dfs(blocks, &mut sizes, &mut post_order, 0, 0)?;

    // Ensure all blocks visited
    ensure!(blocks.len() == post_order.len());

    post_order.reverse();
    let order = post_order;

    let mut block_num_to_order = vec![0; blocks.len()];
    for (i, block) in order.iter().enumerate() {
        block_num_to_order[*block] = i;
    }

    Ok((order, block_num_to_order))
}

// Find block ancestors and detect back edges
// Could be folded into above DFS
//
// Uses the property that for the most part control flow is
// acyclic and the only type of cycles we get are
// enter -> check <-> loop ;
// check -> exit;

fn find_ancestors(blocks: &[SSABlock]) -> Vec<Vec<usize>> {
    let mut ancestors = vec![HashSet::new(); blocks.len()];
    for (block_num, block) in blocks.iter().enumerate() {
        for a in block.exit.referenced_blocks() {
            ancestors[a].insert(block_num);
        }
    }
    return ancestors
        .into_iter()
        .map(|a| a.into_iter().collect())
        .collect();
}

fn expand_used_prev(blocks: &mut [SSABlock], ancestors: &[Vec<usize>]) {
    let mut stack = vec![];

    for (block_num, b) in blocks.iter().enumerate() {
        for uses in &b.final_state.used_prev {
            for ancestor in &ancestors[block_num] {
                stack.push((*ancestor, *uses));
            }
        }
    }

    while let Some((block_num, used_elem)) = stack.pop() {
        if !blocks[block_num].final_state.used_prev.contains(&used_elem) {
            // Note pick will updated used_elems if needed

            if let SSAVar::Prev(uses) = blocks[block_num].final_state.get_subst(used_elem) {
                for ancestor in &ancestors[block_num] {
                    stack.push((*ancestor, uses));
                }
            }
        }
    }

    // Eventually this will settle(*) and used_prev will be the transitive closure of everything used
}

fn relocate_blocks(blocks: &mut [SSABlock]) -> Result<()> {
    let (order, block_num_to_order) = dfs_blocks(blocks)?;
    let ancestors = find_ancestors(blocks);
    expand_used_prev(blocks, &ancestors);

    // Now the actual algorithm to join the blocks together
    // This time it's a BFS starting at block 0.
    //
    // For every block, look at it's ancestors and see if they have different opinions about the
    // value of a given <prev:{}> node.
    //
    // If they do insert a phi node.
    //
    // We deal with back edges by saying they *all* need phi nodes added but not specifying a value
    // yet.
    // After it's done we fix things up

    let is_back_edge = |u, v| block_num_to_order[u] >= block_num_to_order[v];

    #[derive(Clone)]
    struct Patch {
        substitution: SSASubstitutionTarget,
        child_block: usize,
        child_statement: usize,
    }

    let mut patches = vec![vec![]; blocks.len()];

    for &cur_block_num in order.iter() {
        let mut phis = vec![];
        let mut substitutions = HashMap::new();

        let used_prev: Vec<_> = blocks[cur_block_num]
            .final_state
            .used_prev
            .iter()
            .copied()
            .collect();
        for prev_n in used_prev {
            let options: HashMap<_, _> = ancestors[cur_block_num]
                .iter()
                .map(|&ancestor_block_num| {
                    if is_back_edge(ancestor_block_num, cur_block_num) {
                        // Back edges are dealt with later, because they won't be computed
                        // at this point in the search
                        patches[ancestor_block_num].push(Patch {
                            substitution: prev_n,
                            child_block: cur_block_num,
                            child_statement: phis.len(),
                        });
                        (ancestor_block_num, SSAVar::Junk)
                    } else {
                        let prev_value = blocks[ancestor_block_num].final_state.get_subst(prev_n);
                        (ancestor_block_num, prev_value)
                    }
                })
                .collect();

            let first_value = *options.values().next().unwrap();

            if options.values().all(|v| v == &first_value) {
                substitutions.insert(prev_n, first_value);
            } else {
                substitutions.insert(prev_n, SSAVar::Computed(cur_block_num, phis.len()));
                phis.push(options)
            }
        }

        // Ok, we need to make room for any phi nodes by relocating variables offset by the number
        // of phi nodes
        let offset = phis.len();

        if offset > 0 {
            for s in blocks[cur_block_num].statements.iter_mut() {
                if let SSAStatement::Assign(_, var_num, _) = s {
                    *var_num += offset;
                }
            }

            blocks[cur_block_num].modify_ssa_vars(&mut |v| {
                if let SSAVar::Computed(block_num, var_num) = v {
                    if *block_num == cur_block_num {
                        *var_num += offset;
                    }
                }
            });

            // Now we can insert the new phi nodes at the start of the block
            let mut new_statements = vec![];

            for (i, options) in phis.into_iter().enumerate() {
                new_statements.push(SSAStatement::Assign(
                    cur_block_num,
                    i,
                    SSAExpr::Phi(options),
                ));
            }
            new_statements.append(&mut blocks[cur_block_num].statements);
            blocks[cur_block_num].statements = new_statements;
        }

        // Now we've got a value (in substitutions) for everything, we need to perform the
        // substitutions
        blocks[cur_block_num].modify_ssa_vars(&mut |v| match v {
            SSAVar::Prev(t) => match substitutions.get(t) {
                Some(&v_new) => *v = v_new,
                None => (),
            },
            _ => (),
        });
    }

    for (parent_block_num, patches) in patches.into_iter().enumerate() {
        for Patch {
            substitution,
            child_block,
            child_statement,
        } in patches
        {
            let new_value = blocks[parent_block_num].final_state.get_subst(substitution);
            match &mut blocks[child_block].statements[child_statement] {
                SSAStatement::Assign(_, _, SSAExpr::Phi(nodes)) => {
                    nodes.insert(parent_block_num, new_value);
                }
                _ => bail!("Expected phi node!"),
            }
        }
    }

    Ok(())
}
