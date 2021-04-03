use super::types::*;
use crate::{Instruction, InstructionIterator, Opcode};
use anyhow::{anyhow, ensure, Result};
use std::collections::{HashMap, HashSet};

// Conversion of a closure

pub fn parse_to_basic_blocks(code: &[i32], start_offset: usize) -> Result<BasicClosure> {
    let mut block_starts = HashSet::new();
    block_starts.insert(start_offset);
    find_block_starts_dfs(code, start_offset, &mut block_starts)?;

    convert_dfs(code, start_offset, &block_starts)
}

// Find all referenced locations which are the starts of blocks
// This is the first of two DFS's we do - but it's the simpler one because all it needs to do
// is find referenced locations where loop back edges join up
fn find_block_starts_dfs(
    code: &[i32],
    start_offset: usize,
    seen: &mut HashSet<usize>,
) -> Result<()> {
    let mut inst_iter = InstructionIterator::new_from_offset(
        (&code[start_offset..code.len()]).iter().copied(),
        start_offset,
    );

    let mut visit = |label_offset: usize| -> Result<()> {
        if !seen.contains(&label_offset) {
            seen.insert(label_offset);
            find_block_starts_dfs(code, label_offset, seen)?;
        }

        Ok(())
    };

    let mut to_visit = vec![];

    while let Some(instr) = inst_iter.next() {
        let instr = instr?;
        match instr {
            Instruction::LabelDef(_)
            | Instruction::PushRetAddr(_)
            | Instruction::Closure(_, _)
            | Instruction::ClosureRec(_, _) => {
                continue;
            }

            _ => {}
        }
        to_visit.clear();
        instr.visit_labels(|l| {
            to_visit.push(l.0);
        });
        for l in &to_visit {
            visit(*l)?;
        }

        //  Check for instructions that end the basic block
        match instr {
            // These can fallthrough to the next instruction
            Instruction::BranchIf(_)
            | Instruction::BranchIfNot(_)
            | Instruction::BranchCmp(_, _, _)
            | Instruction::PushTrap(_) => {
                visit(inst_iter.current_position())?;
                break;
            }

            // These don't follow through but do end the block
            Instruction::Branch(_)
            | Instruction::ApplyTerm(_, _)
            | Instruction::Return(_)
            | Instruction::Switch(_, _)
            | Instruction::Raise(_)
            | Instruction::Stop => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

// This is a monster DFS that works out stack sizes, validates invariants, works out the block
// ordering (reverse post-order), works out predecessors and maps instructions to the basic block
// form!
//
// It requires knowing the back edge block starts for while loops only
fn convert_dfs(
    code: &[i32],
    entrypoint: usize,
    block_starts: &HashSet<usize>,
) -> Result<BasicClosure> {
    // Check for the first instruction to work out arity
    let arity = if let Some(Opcode::Grab) = Opcode::from_i32(code[entrypoint]) {
        code[entrypoint + 1] as usize + 1
    } else {
        1
    };

    let mut search_state = SearchState {
        code,
        block_starts,
        seen: HashMap::new(),
        used_closures: Vec::new(),
        finished: Vec::new(),
        max_stack_size: 0,
        has_trap_handlers: false,
    };
    search_state.visit(entrypoint, None, arity as u32, BasicBlockType::First)?;

    // Now we've visited everything, we can actually create the blocks
    let mut blocks = vec![];
    let mut offset_to_block_id_map = HashMap::new();

    for (block_id, finished_block) in search_state.finished.into_iter().rev().enumerate() {
        let pending_block = search_state.seen.remove(&finished_block.offset).unwrap();
        blocks.push(BasicBlock {
            block_id,
            predecessors: pending_block.predecessors.iter().copied().collect(),
            block_type: pending_block.block_type,
            instructions: finished_block.instructions,
            exit: finished_block.exit,
            start_stack_size: pending_block.start_stack_size,
            end_stack_size: finished_block.end_stack_size,
            sealed_blocks: Vec::new(),
        });
        offset_to_block_id_map.insert(finished_block.offset, block_id);
    }

    // We now need to relocate offsets into block ids
    let relocate_offset = |offset: &mut usize| {
        let block_id = *offset_to_block_id_map.get(offset).unwrap();
        *offset = block_id;
    };

    for block in &mut blocks {
        block.exit.modify_block_labels(relocate_offset);
        block.predecessors.iter_mut().for_each(relocate_offset);
    }

    let mut max_preds = vec![];

    // Work out sealed blocks
    for block in blocks.iter() {
        max_preds.push(block.predecessors.iter().max().copied());
    }

    for (block_id, max_pred_opt) in max_preds.into_iter().enumerate() {
        if let Some(max_pred) = max_pred_opt {
            blocks[max_pred].sealed_blocks.push(block_id)
        }
    }

    Ok(BasicClosure {
        arity,
        blocks,
        used_closures: search_state.used_closures,
        max_stack_size: search_state.max_stack_size,
        has_trap_handlers: search_state.has_trap_handlers,
    })
}

struct SearchState<'a> {
    code: &'a [i32],
    block_starts: &'a HashSet<usize>,
    seen: HashMap<usize, PendingBlock>,
    used_closures: Vec<usize>,
    max_stack_size: u32,
    has_trap_handlers: bool,
    // During the search this will hold things in post-order
    finished: Vec<FinishedBlock>,
}

// This big process/DFS has so much state we're tracking that I've made a struct to hold it
struct PendingBlock {
    block_type: BasicBlockType,
    start_stack_size: u32,
    predecessors: HashSet<usize>,
}

struct FinishedBlock {
    offset: usize,
    instructions: Vec<BasicBlockInstruction>,
    exit: BasicBlockExit,
    end_stack_size: u32,
}

impl<'a> SearchState<'a> {
    fn visit(
        &mut self,
        entrypoint: usize,
        predecessor: Option<usize>,
        start_stack_size: u32,
        block_type: BasicBlockType,
    ) -> Result<()> {
        use BasicBlockExit::*;
        use BasicBlockInstruction::*;
        // Deal with if this block has already been seen - check stack start lines up
        // and add to predecessors
        if let Some(existing) = self.seen.get_mut(&entrypoint) {
            ensure!(existing.start_stack_size == start_stack_size);
            ensure!(existing.block_type == block_type);

            if let Some(p) = predecessor {
                existing.predecessors.insert(p);
            }

            return Ok(());
        }

        // Otherwise we're seeing the block for the first time - make a seen entry
        let mut predecessors = HashSet::new();
        if let Some(p) = predecessor {
            predecessors.insert(p);
        }
        self.seen.insert(
            entrypoint,
            PendingBlock {
                block_type,
                start_stack_size,
                predecessors,
            },
        );

        // Now we iterate through all the instructions until we find something representing an
        // exit
        let mut stack_size = start_stack_size;
        let mut instructions = vec![];
        let mut exit = None;

        let mut inst_iter = InstructionIterator::new_from_offset(
            (&self.code[entrypoint..self.code.len()]).iter().copied(),
            entrypoint,
        );

        loop {
            if stack_size > self.max_stack_size {
                self.max_stack_size = stack_size;
            }

            match exit {
                None => {
                    let instr = inst_iter
                        .next()
                        .ok_or_else(|| anyhow!("Expected an instruction"))??;

                    match instr {
                        Instruction::LabelDef(l) => {
                            let cur_offset = l.0;
                            // Check if we need to insert a jump because of a back edge
                            if cur_offset != entrypoint && self.block_starts.contains(&cur_offset) {
                                // Emit a jump
                                self.visit(
                                    cur_offset,
                                    Some(entrypoint),
                                    stack_size,
                                    BasicBlockType::Normal,
                                )?;
                                exit = Some(BasicBlockExit::Branch(cur_offset));
                            }
                        }
                        Instruction::Acc(i) => {
                            instructions.push(Acc(i));
                        }
                        Instruction::EnvAcc(i) => {
                            instructions.push(EnvAcc(i));
                        }
                        Instruction::Push => {
                            stack_size += 1;
                            instructions.push(Push);
                        }
                        Instruction::Pop(n) => {
                            ensure!(stack_size >= n);
                            instructions.push(Pop(n));
                            stack_size -= n;
                        }
                        Instruction::Assign(n) => {
                            ensure!(stack_size > n);
                            instructions.push(Assign(n));
                        }
                        Instruction::PushRetAddr(_) => {
                            stack_size += 3;
                            instructions.push(PushRetAddr)
                        }
                        Instruction::Apply1 => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(Apply1);
                        }
                        Instruction::Apply2 => {
                            ensure!(stack_size >= 2);
                            stack_size -= 2;
                            instructions.push(Apply2);
                        }
                        Instruction::Apply3 => {
                            ensure!(stack_size >= 3);
                            stack_size -= 3;
                            instructions.push(Apply3);
                        }
                        Instruction::Apply(nargs) => {
                            // 3 for the return frame from PushRetAddr
                            ensure!(stack_size >= nargs + 3);
                            stack_size -= nargs + 3;
                            instructions.push(Apply(nargs))
                        }
                        Instruction::ApplyTerm(nargs, slot_size) => {
                            ensure!(stack_size == slot_size);
                            stack_size = 0;
                            exit = Some(TailCall {
                                args: nargs,
                                to_pop: slot_size,
                            });
                        }
                        Instruction::Return(to_pop) => {
                            ensure!(stack_size == to_pop);
                            stack_size -= to_pop;
                            exit = Some(Return(to_pop));
                        }
                        // Ignore restart and grab
                        Instruction::Restart => {}
                        Instruction::Grab(_) => {}
                        Instruction::Closure(l, nvars) => {
                            if nvars > 0 {
                                ensure!(stack_size >= nvars - 1);
                                stack_size -= nvars - 1;
                            }
                            instructions.push(Closure(l.0, nvars));
                            self.used_closures.push(l.0);
                        }
                        Instruction::ClosureRec(closures, nvars) => {
                            if nvars > 0 {
                                ensure!(stack_size >= nvars - 1);
                                stack_size -= nvars - 1;
                            }
                            stack_size += closures.len() as u32;
                            instructions.push(ClosureRec(
                                closures
                                    .into_iter()
                                    .map(|x| {
                                        let offset = x.0;
                                        self.used_closures.push(offset);
                                        offset
                                    })
                                    .collect(),
                                nvars,
                            ));
                        }
                        Instruction::OffsetClosure(i) => {
                            instructions.push(OffsetClosure(i));
                        }
                        Instruction::GetGlobal(g) => {
                            instructions.push(GetGlobal(g));
                        }
                        Instruction::SetGlobal(g) => {
                            instructions.push(SetGlobal(g));
                        }
                        Instruction::Const(i) => {
                            instructions.push(Const(i));
                        }
                        Instruction::MakeBlock(size, tag) => {
                            let to_pop = if size > 0 { size - 1 } else { 0 };
                            ensure!(stack_size >= to_pop);
                            stack_size -= to_pop;
                            instructions.push(MakeBlock(size, tag));
                        }
                        Instruction::MakeFloatBlock(size) => {
                            let to_pop = if size > 0 { size - 1 } else { 0 };
                            ensure!(stack_size >= to_pop);
                            stack_size -= to_pop;
                            instructions.push(MakeFloatBlock(size));
                        }
                        Instruction::GetField(n) => {
                            instructions.push(GetField(n));
                        }
                        Instruction::SetField(n) => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(SetField(n));
                        }
                        Instruction::GetFloatField(n) => {
                            instructions.push(GetFloatField(n));
                        }
                        Instruction::SetFloatField(n) => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(SetFloatField(n));
                        }
                        Instruction::VecTLength => {
                            instructions.push(VecTLength);
                        }
                        Instruction::GetVecTItem => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(GetVecTItem);
                        }
                        Instruction::SetVecTItem => {
                            ensure!(stack_size >= 2);
                            stack_size -= 2;
                            instructions.push(SetVecTItem);
                        }
                        Instruction::GetBytesChar => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(GetBytesChar);
                        }
                        Instruction::SetBytesChar => {
                            ensure!(stack_size >= 2);
                            stack_size -= 2;
                            instructions.push(SetBytesChar);
                        }
                        Instruction::Branch(to) => {
                            let offset = to.0;
                            self.visit(
                                offset,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            exit = Some(Branch(offset));
                        }
                        Instruction::BranchIf(to) => {
                            let then_block = to.0;
                            let else_block = inst_iter.current_position();
                            self.visit(
                                else_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            self.visit(
                                then_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            exit = Some(BranchIf {
                                then_block,
                                else_block,
                            });
                        }
                        Instruction::BranchIfNot(to) => {
                            let then_block = inst_iter.current_position();
                            let else_block = to.0;
                            self.visit(
                                else_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            self.visit(
                                then_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            exit = Some(BranchIf {
                                then_block,
                                else_block,
                            });
                        }
                        Instruction::Switch(ints, tags) => {
                            let mut ints_r = vec![];
                            for l in ints {
                                let offset = l.0;
                                self.visit(
                                    offset,
                                    Some(entrypoint),
                                    stack_size,
                                    BasicBlockType::Normal,
                                )?;
                                ints_r.push(offset);
                            }
                            let mut tags_r = vec![];
                            for l in tags {
                                let offset = l.0;
                                self.visit(
                                    offset,
                                    Some(entrypoint),
                                    stack_size,
                                    BasicBlockType::Normal,
                                )?;
                                tags_r.push(offset);
                            }

                            exit = Some(Switch {
                                ints: ints_r,
                                tags: tags_r,
                            })
                        }
                        Instruction::BoolNot => {
                            instructions.push(BoolNot);
                        }
                        Instruction::PushTrap(to) => {
                            self.has_trap_handlers = true;
                            let trap = to.0;
                            let normal = inst_iter.current_position();
                            self.visit(trap, Some(entrypoint), stack_size, BasicBlockType::Normal)?;
                            self.visit(
                                normal,
                                Some(entrypoint),
                                stack_size + 4,
                                BasicBlockType::Normal,
                            )?;
                            stack_size += 4; // Consider normal as the end stack size
                            exit = Some(PushTrap { normal, trap });
                        }
                        Instruction::PopTrap => {
                            ensure!(stack_size >= 4);
                            stack_size -= 4;
                        }
                        Instruction::Raise(r) => {
                            exit = Some(Raise(r));
                        }
                        Instruction::CheckSignals => {
                            instructions.push(CheckSignals);
                        }
                        Instruction::CCall1(id) => {
                            instructions.push(CCall1(id));
                        }
                        Instruction::CCall2(id) => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(CCall2(id));
                        }
                        Instruction::CCall3(id) => {
                            ensure!(stack_size >= 2);
                            stack_size -= 2;
                            instructions.push(CCall3(id));
                        }
                        Instruction::CCall4(id) => {
                            ensure!(stack_size >= 3);
                            stack_size -= 3;
                            instructions.push(CCall4(id));
                        }
                        Instruction::CCall5(id) => {
                            ensure!(stack_size >= 4);
                            stack_size -= 4;
                            instructions.push(CCall5(id));
                        }
                        Instruction::CCallN(nargs, id) => {
                            let to_pop = nargs - 1;
                            ensure!(stack_size >= to_pop);
                            stack_size -= to_pop;
                            instructions.push(CCallN { nargs, id })
                        }
                        Instruction::ArithInt(op) => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(ArithInt(op));
                        }
                        Instruction::NegInt => {
                            instructions.push(NegInt);
                        }
                        Instruction::IntCmp(comp) => {
                            ensure!(stack_size >= 1);
                            stack_size -= 1;
                            instructions.push(IntCmp(comp));
                        }
                        Instruction::BranchCmp(cmp, constant, to) => {
                            let then_block = to.0;
                            let else_block = inst_iter.current_position();
                            self.visit(
                                then_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            self.visit(
                                else_block,
                                Some(entrypoint),
                                stack_size,
                                BasicBlockType::Normal,
                            )?;
                            exit = Some(BranchCmp {
                                cmp,
                                constant,
                                then_block,
                                else_block,
                            });
                        }
                        Instruction::OffsetInt(n) => {
                            instructions.push(OffsetInt(n));
                        }
                        Instruction::OffsetRef(r) => {
                            instructions.push(OffsetRef(r));
                        }
                        Instruction::IsInt => {
                            instructions.push(IsInt);
                        }
                        Instruction::GetMethod => {
                            ensure!(stack_size >= 1);
                            instructions.push(GetMethod);
                        }
                        Instruction::SetupForPubMet(n) => {
                            stack_size += 1;
                            instructions.push(Push);
                            instructions.push(Const(n));
                        }
                        Instruction::GetDynMet => {
                            ensure!(stack_size >= 1);
                            instructions.push(GetDynMet);
                        }
                        Instruction::Stop => {
                            ensure!(stack_size == 1);
                            exit = Some(Stop);
                        }
                        // Ignore break/event
                        Instruction::Break => {}
                        Instruction::Event => {}
                    }
                }
                Some(exit) => {
                    self.finished.push(FinishedBlock {
                        offset: entrypoint,
                        instructions,
                        exit,
                        end_stack_size: stack_size,
                    });
                    return Ok(());
                }
            }
        }
    }
}
