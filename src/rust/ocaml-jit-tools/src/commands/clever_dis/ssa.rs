use crate::commands::clever_dis::data::{Block, BlockExit, Closure, Program};
use ocaml_jit_shared::{ArithOp, Comp, Instruction, Primitive, RaiseKind};
use std::cmp::max;
use std::env::args;
use std::fmt::{Binary, Display, Formatter};

fn display_array_single_line<T: Display>(f: &mut Formatter, array: &[T]) -> std::fmt::Result {
    let mut first = true;
    write!(f, "[")?;
    for v in array {
        if first {
            first = false;
        } else {
            write!(f, ", ")?;
        }

        write!(f, "{}", v)?;
    }
    write!(f, "]")?;

    Ok(())
}

#[derive(Debug)]
pub struct SSABlock {
    statements: Vec<SSAStatement>,
    exit: SSAExit,
}

impl Display for SSABlock {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for statement in &self.statements {
            writeln!(f, "{}", statement)?;
        }

        writeln!(f, "Exit: {}", &self.exit)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SSAVar {
    Arg(usize),
    Env(usize),
    Computed(usize),
    OffsetClosure(isize),
    Const(i32),
    Unit,
    Ret1,
    Ret2,
    Ret3,
}

impl Display for SSAVar {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAVar::Arg(i) => write!(f, "a{}", i),
            SSAVar::Env(i) => write!(f, "e{}", i),
            SSAVar::Computed(i) => write!(f, "v{}", i),
            SSAVar::OffsetClosure(i) => write!(f, "oc<{}>", i),
            SSAVar::Const(i) => write!(f, "{}", i),
            SSAVar::Unit => write!(f, "()"),
            SSAVar::Ret1 => write!(f, "<ret1>"),
            SSAVar::Ret2 => write!(f, "<ret2>"),
            SSAVar::Ret3 => write!(f, "<ret3>"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UnaryFloatOp {
    Neg,
    Sqrt,
}

impl Display for UnaryFloatOp {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            UnaryFloatOp::Neg => write!(f, "neg.f"),
            UnaryFloatOp::Sqrt => write!(f, "sqrt.f"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BinaryFloatOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl Display for BinaryFloatOp {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            BinaryFloatOp::Add => write!(f, "add.f"),
            BinaryFloatOp::Sub => write!(f, "sub.f"),
            BinaryFloatOp::Mul => write!(f, "mul.f"),
            BinaryFloatOp::Div => write!(f, "div.f"),
        }
    }
}

#[derive(Debug)]
pub enum SSAExpr {
    Apply(SSAVar, Vec<SSAVar>),
    GetGlobal(usize),
    GetField(SSAVar, usize),
    OffsetInt(SSAVar, i32),
    UnaryFloat(UnaryFloatOp, SSAVar),
    BinaryFloat(BinaryFloatOp, SSAVar, SSAVar),
    MakeBlock {
        tag: u8,
        vars: Vec<SSAVar>,
    },
    Closure {
        code: usize,
        vars: Vec<SSAVar>,
    },
    ClosureRec {
        codes: Vec<usize>,
        vars: Vec<SSAVar>,
    },
    CCall {
        primitive_id: usize,
        vars: Vec<SSAVar>,
    },
}

impl Display for SSAExpr {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAExpr::Apply(closure, args) => {
                write!(f, "apply {} ", closure)?;

                display_array_single_line(f, args)?;
            }
            SSAExpr::GetGlobal(n) => {
                write!(f, "g{}", n)?;
            }
            SSAExpr::GetField(v, i) => {
                write!(f, "{}[{}]", v, i)?;
            }
            SSAExpr::OffsetInt(v, i) => {
                write!(f, "{} + {}", v, i)?;
            }
            SSAExpr::BinaryFloat(op, a, b) => {
                write!(f, "{} {} {}", op, a, b)?;
            }
            SSAExpr::UnaryFloat(op, x) => {
                write!(f, "{} {}", op, x)?;
            }
            SSAExpr::MakeBlock { tag, vars } => {
                write!(f, "make block tag:{} vars:", tag)?;
                display_array_single_line(f, vars)?;
            }
            SSAExpr::Closure { code, vars } => {
                write!(f, "make closure code:{} vars:", code)?;
                display_array_single_line(f, vars)?;
            }
            SSAExpr::ClosureRec { codes, vars } => {
                write!(f, "make rec closure codes:")?;
                display_array_single_line(f, codes)?;
                write!(f, " vars:")?;
                display_array_single_line(f, vars)?;
            }
            SSAExpr::CCall { primitive_id, vars } => {
                write!(f, "ccall {} ", primitive_id)?;
                display_array_single_line(f, vars)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum SSAStatement {
    Assign(usize, SSAExpr),
    PopTrap,
    CheckSignals,
    SetGlobal(usize, SSAVar),
}

impl Display for SSAStatement {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAStatement::Assign(i, expr) => {
                write!(f, "v{} = {}", i, expr)?;
            }
            SSAStatement::PopTrap => {
                write!(f, "pop trap")?;
            }
            SSAStatement::CheckSignals => {
                write!(f, "check signals")?;
            }
            SSAStatement::SetGlobal(n, v) => {
                write!(f, "set g{} = {}", n, v)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum SSAExit {
    TempIDK,
    Stop,
    Jump(usize),
    Return(SSAVar),
}

impl Display for SSAExit {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAExit::TempIDK => {
                write!(f, "Temp IDK?")?;
            }
            SSAExit::Stop => {
                write!(f, "stop")?;
            }
            SSAExit::Jump(block) => {
                write!(f, "jump {}", block)?;
            }
            SSAExit::Return(v) => {
                write!(f, "return {}", v)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct State {
    stack: Vec<SSAVar>,
    acc: SSAVar,
    stack_start: usize,
}

impl State {
    fn pick(&self, n: usize) -> SSAVar {
        if n < self.stack.len() {
            self.stack[self.stack.len() - 1 - n]
        } else {
            let arg_offset = n - self.stack.len();
            return SSAVar::Arg(self.stack_start + arg_offset);
        }
    }

    fn pop(&mut self, count: usize) {
        let to_keep_in_stack = max(self.stack.len() as isize - count as isize, 0) as usize;
        let to_remove_from_stack = self.stack.len() - to_keep_in_stack;
        self.stack.truncate(to_keep_in_stack);

        let remaining = count - to_remove_from_stack;
        self.stack_start += remaining;
    }

    fn push(&mut self, entry: SSAVar) {
        self.stack.push(entry);
    }

    fn assign(&mut self, index: usize, entry: SSAVar) {
        if index >= self.stack.len() {
            let todo = index - self.stack.len() + 1;
            self.stack_start += todo;
            let mut tmp_stack = vec![];
            for i in 1..=todo {
                tmp_stack.push(SSAVar::Arg(self.stack_start - i - 1));
            }

            std::mem::swap(&mut self.stack, &mut tmp_stack);
            self.stack.extend(tmp_stack);
        }

        let length = self.stack.len();
        self.stack[length - 1 - index] = entry;
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        writeln!(
            f,
            "TOS: a{}, a{}, ..",
            self.stack_start,
            self.stack_start + 1
        )?;
        writeln!(f, "Stack: [")?;

        for entry in &self.stack {
            writeln!(f, "    {}", entry)?;
        }
        writeln!(f, "]")?;

        writeln!(f, "Final acc: {}", self.acc)?;

        Ok(())
    }
}

struct Vars {
    statements: Vec<SSAStatement>,
    num_assignments: usize,
}

impl Vars {
    fn new() -> Vars {
        Vars {
            statements: Vec::new(),
            num_assignments: 0,
        }
    }

    fn add_statement(&mut self, statement: SSAStatement) {
        self.statements.push(statement);
    }

    fn add_assignment(&mut self, expr: SSAExpr) -> SSAVar {
        let assignment_num = self.num_assignments;
        self.num_assignments += 1;

        self.add_statement(SSAStatement::Assign(assignment_num, expr));
        SSAVar::Computed(assignment_num)
    }
}

fn translate_block(block: &Block) -> (SSABlock, State) {
    assert!(block.instructions.len() > 0);
    let last_instr_idx = block.instructions.len() - 1;

    let mut vars_d = Vars::new();
    let mut state_d = State {
        stack: vec![],
        acc: SSAVar::Arg(0),
        stack_start: 1,
    };

    let vars = &mut vars_d;
    let state = &mut state_d;

    for instr in &block.instructions[0..last_instr_idx] {
        match instr {
            Instruction::LabelDef(_) => {}
            Instruction::Acc(n) => {
                state.acc = state.pick(*n as usize);
            }
            Instruction::EnvAcc(n) => {
                state.acc = SSAVar::Env(*n as usize);
            }
            Instruction::Push => {
                state.push(state.acc);
            }
            Instruction::Pop(n) => {
                state.pop(*n as usize);
            }
            Instruction::Assign(n) => {
                state.assign(*n as usize, state.acc);
            }
            Instruction::PushRetAddr(_) => {
                state.push(SSAVar::Ret3);
                state.push(SSAVar::Ret2);
                state.push(SSAVar::Ret1);
            }
            Instruction::Apply1 => {
                state.acc = vars.add_assignment(SSAExpr::Apply(state.acc, vec![state.pick(0)]));
                state.pop(1);
            }
            Instruction::Apply2 => {
                state.acc = vars.add_assignment(SSAExpr::Apply(
                    state.acc,
                    vec![state.pick(1), state.pick(0)],
                ));
                state.pop(2);
            }
            Instruction::Apply3 => {
                state.acc = vars.add_assignment(SSAExpr::Apply(
                    state.acc,
                    vec![state.pick(2), state.pick(1), state.pick(0)],
                ));
                state.pop(3);
            }
            Instruction::Apply(nvars) => {
                let nvars = *nvars as usize;
                assert!(nvars > 3);

                let passed_vars = (nvars - 2..=0).map(|n| state.pick(n)).collect();

                assert_eq!(state.pick(0), SSAVar::Ret3);
                assert_eq!(state.pick(1), SSAVar::Ret3);
                assert_eq!(state.pick(2), SSAVar::Ret3);
                state.pop(3);

                state.acc = vars.add_assignment(SSAExpr::Apply(state.acc, passed_vars));
                // todo pop
            }
            Instruction::ApplyTerm(_, _) => {
                panic!("Apply term should be last call in a block!");
            }
            Instruction::Return(n) => {
                panic!("Return should be last call in a block!");
            }
            Instruction::Restart => {}
            Instruction::Grab(_, _) => {}
            Instruction::Closure(loc, nvars) => {
                let nvars = *nvars as usize;
                if nvars > 0 {
                    state.push(state.acc);
                }

                state.acc = vars.add_assignment(SSAExpr::Closure {
                    code: *loc,
                    vars: (0..nvars).map(|i| state.pick(i)).collect(),
                });

                state.pop(nvars);
            }
            Instruction::ClosureRec(locs, nvars) => {
                let nvars = *nvars as usize;
                if nvars > 0 {
                    state.push(state.acc);
                }

                state.acc = vars.add_assignment(SSAExpr::ClosureRec {
                    codes: locs.clone(),
                    vars: (0..nvars).map(|i| state.pick(i)).collect(),
                });

                state.pop(nvars);
                // I have no idea why closure-rec does this but normal closure doesn't
                state.push(state.acc);
            }
            Instruction::OffsetClosure(i) => {
                state.acc = SSAVar::OffsetClosure(*i as isize);
            }
            Instruction::GetGlobal(n) => {
                state.acc = vars.add_assignment(SSAExpr::GetGlobal(*n as usize));
            }
            Instruction::SetGlobal(n) => {
                vars.add_statement(SSAStatement::SetGlobal(*n as usize, state.acc));
                state.acc = SSAVar::Unit;
            }
            Instruction::Const(v) => {
                state.acc = SSAVar::Const(*v);
            }
            Instruction::MakeBlock(count, tag) => {
                let count = *count as usize;
                let tag = *tag;

                if count > 0 {
                    state.push(state.acc);
                }

                state.acc = vars.add_assignment(SSAExpr::MakeBlock {
                    tag,
                    vars: (0..count).map(|i| state.pick(i)).collect(),
                });

                state.pop(count);
            }
            Instruction::MakeFloatBlock(_) => {}
            Instruction::GetField(n) => {
                state.acc = vars.add_assignment(SSAExpr::GetField(state.acc, *n as usize));
            }
            Instruction::SetField(_) => {}
            Instruction::GetFloatField(_) => {}
            Instruction::SetFloatField(_) => {}
            Instruction::VecTLength => {}
            Instruction::GetVecTItem => {}
            Instruction::SetVecTItem => {}
            Instruction::GetBytesChar => {}
            Instruction::SetBytesChar => {}
            Instruction::Branch(_) => {}
            Instruction::BranchIf(_) => {}
            Instruction::BranchIfNot(_) => {}
            Instruction::Switch(_, _) => {}
            Instruction::BoolNot => {}
            Instruction::PushTrap(_) => {}
            Instruction::PopTrap => {}
            Instruction::Raise(_) => {}
            Instruction::CheckSignals => {
                vars.add_statement(SSAStatement::CheckSignals);
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
            Instruction::ArithInt(_) => {}
            Instruction::NegInt => {}
            Instruction::IntCmp(_) => {}
            Instruction::BranchCmp(_, _, _) => {}
            Instruction::OffsetInt(n) => {
                state.acc = vars.add_assignment(SSAExpr::OffsetInt(state.acc, *n))
            }
            Instruction::OffsetRef(_) => {}
            Instruction::IsInt => {}
            Instruction::GetMethod => {}
            Instruction::SetupForPubMet(_) => {}
            Instruction::GetDynMet => {}
            Instruction::Stop => {}
            Instruction::Break => {}
            Instruction::Event => {}
        }
    }

    let last_instruction = block.instructions.last().unwrap();
    let exit = match (&block.exit, last_instruction) {
        (BlockExit::Stop, Instruction::Stop) => SSAExit::Stop,
        (BlockExit::UnconditionalJump(n1), Instruction::Branch(n2)) => {
            assert_eq!(n1, n2);
            SSAExit::Jump(*n1)
        }
        (BlockExit::Return, Instruction::Return(count)) => {
            state.pop(*count as usize);
            SSAExit::Return(state.acc)
        }

        _ => SSAExit::TempIDK,
    };

    (
        SSABlock {
            statements: vars_d.statements,
            exit,
        },
        state_d,
    )
}

// Shared utilities for parser

fn c_call(state: &mut State, vars: &mut Vars, count: usize, primitive_id: &u32) {
    state.push(state.acc);

    state.acc = vars.add_assignment(SSAExpr::CCall {
        primitive_id: *primitive_id as usize,
        vars: (0..count).map(|i| state.pick(i)).collect(),
    });
    state.pop(count);
}

fn unary_float(state: &mut State, vars: &mut Vars, op: UnaryFloatOp) {
    state.acc = vars.add_assignment(SSAExpr::UnaryFloat(op, state.acc));
}

fn binary_float(state: &mut State, vars: &mut Vars, op: BinaryFloatOp) {
    state.acc = vars.add_assignment(SSAExpr::BinaryFloat(op, state.acc, state.pick(0)));
    state.pop(1);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::commands::clever_dis::data::BlockExit;
    use expect_test::{expect, Expect};

    use ocaml_jit_shared::Primitive;
    use std::fmt::Debug;
    use Instruction::*;

    fn check_debug<T: Debug>(thing: &T, expected: Expect) {
        expected.assert_eq(&format!("{:?}", thing));
    }

    fn check_debug_pretty<T: Debug>(thing: &T, expected: Expect) {
        expected.assert_eq(&format!("{:#?}", thing));
    }

    #[test]
    fn test_state() {
        let start_state = State {
            stack: vec![],
            acc: SSAVar::Arg(0),
            stack_start: 1,
        };

        // Basic pick behaviour
        {
            let mut state = start_state.clone();

            // Make sure picking has the correct behaviour
            check_debug(&state.stack, expect![[r#"[]"#]]);
            check_debug(&state.pick(0), expect![[r#"Arg(1)"#]]);
            check_debug(&state.pick(1), expect![[r#"Arg(2)"#]]);
            check_debug(&state.pick(2), expect![[r#"Arg(3)"#]]);

            // Push something
            state.push(SSAVar::Computed(0));
            check_debug(&state.stack, expect![[r#"[Computed(0)]"#]]);
            check_debug(&state.pick(0), expect![[r#"Computed(0)"#]]);
            check_debug(&state.pick(1), expect![[r#"Arg(1)"#]]);
            check_debug(&state.pick(2), expect![[r#"Arg(2)"#]]);
        }

        // Popping with a completely empty stack
        {
            let mut state = start_state.clone();

            check_debug(
                &state,
                expect![[r#"State { stack: [], acc: Arg(0), stack_start: 1 }"#]],
            );

            state.pop(3);
            check_debug(
                &state,
                expect![[r#"State { stack: [], acc: Arg(0), stack_start: 4 }"#]],
            );
            check_debug(&state.pick(0), expect![[r#"Arg(4)"#]]);
            check_debug(&state.pick(1), expect![[r#"Arg(5)"#]]);
            check_debug(&state.pick(2), expect![[r#"Arg(6)"#]]);
        }

        // Push a few before popping from the pushed things
        {
            let mut state = start_state.clone();

            state.push(SSAVar::Computed(0));
            state.push(SSAVar::Computed(1));
            state.push(SSAVar::Computed(2));
            state.push(SSAVar::Computed(3));
            check_debug_pretty(
                &state,
                expect![[r#"
                    State {
                        stack: [
                            Computed(
                                0,
                            ),
                            Computed(
                                1,
                            ),
                            Computed(
                                2,
                            ),
                            Computed(
                                3,
                            ),
                        ],
                        acc: Arg(
                            0,
                        ),
                        stack_start: 1,
                    }"#]],
            );

            state.pop(2);
            check_debug(
                &state,
                expect![[
                    r#"State { stack: [Computed(0), Computed(1)], acc: Arg(0), stack_start: 1 }"#
                ]],
            );

            state.pop(3);
            check_debug(
                &state,
                expect![[r#"State { stack: [], acc: Arg(0), stack_start: 2 }"#]],
            );
        }
        // Assignments 1
        {
            let mut state = start_state.clone();
            state.assign(0, SSAVar::Computed(12));
            check_debug(
                &state,
                expect![[r#"State { stack: [Computed(12)], acc: Arg(0), stack_start: 2 }"#]],
            );
        }

        // Assignments 2
        {
            let mut state = start_state.clone();
            state.push(SSAVar::Computed(12));
            check_debug(
                &state,
                expect![[r#"State { stack: [Computed(12)], acc: Arg(0), stack_start: 1 }"#]],
            );

            state.assign(0, SSAVar::Computed(23));
            check_debug(
                &state,
                expect![[r#"State { stack: [Computed(23)], acc: Arg(0), stack_start: 1 }"#]],
            );

            state.assign(1, SSAVar::Computed(24));
            check_debug(
                &state,
                expect![[
                    r#"State { stack: [Computed(24), Computed(23)], acc: Arg(0), stack_start: 2 }"#
                ]],
            );

            state.assign(5, SSAVar::Computed(25));
            check_debug_pretty(
                &state,
                expect![[r#"
                    State {
                        stack: [
                            Computed(
                                25,
                            ),
                            Arg(
                                3,
                            ),
                            Arg(
                                2,
                            ),
                            Arg(
                                1,
                            ),
                            Computed(
                                24,
                            ),
                            Computed(
                                23,
                            ),
                        ],
                        acc: Arg(
                            0,
                        ),
                        stack_start: 6,
                    }"#]],
            );
        }
    }

    #[test]
    fn test_block_translation() {
        fn check(instructions: Vec<Instruction<usize>>, exit: BlockExit, expected: Expect) {
            let block = Block {
                instructions,
                exit,
                closures: vec![],
                traps: vec![],
            };

            let (ssa_block, final_state) = translate_block(&block);
            let actual = format!("Block:\n{}\n{}", ssa_block, final_state);
            expected.assert_eq(&actual);
        }

        check(
            vec![
                CheckSignals,
                Acc(0),
                Push,
                GetGlobal(310),
                Push,
                GetGlobal(308),
                GetField(1),
                Apply2,
                Acc(0),
                OffsetInt(1),
                Assign(0),
            ],
            BlockExit::UnconditionalJump(1),
            expect![[r#"
                Block:
                check signals
                v0 = g310
                v1 = g308
                v2 = v1[1]
                v3 = apply v2 [a1, v0]
                v4 = a1 + 1
                Exit: Temp IDK?

                TOS: a1, a2, ..
                Stack: [
                ]
                Final acc: v4
            "#]],
        );

        check(
            vec![
                Acc(0),
                GetField(0),
                Push,
                OffsetClosure(0),
                Apply1,
                MakeBlock(1, 2),
                Return(2),
            ],
            BlockExit::Return,
            expect![[r#"
                Block:
                v0 = a1[0]
                v1 = apply oc<0> [v0]
                v2 = make block tag:2 vars:[v1]
                Exit: return v2

                TOS: a3, a4, ..
                Stack: [
                ]
                Final acc: v2
            "#]],
        );

        check(
            vec![
                ClosureRec(vec![1], 0),
                ClosureRec(vec![2], 0),
                ClosureRec(vec![3], 0),
                Acc(0),
                Push,
                Acc(3),
                Push,
                Acc(3),
                MakeBlock(3, 0),
                Pop(3),
                SetGlobal(12),
                Branch(2),
            ],
            BlockExit::UnconditionalJump(2),
            expect![[r#"
                Block:
                v0 = make rec closure codes:[1] vars:[]
                v1 = make rec closure codes:[2] vars:[]
                v2 = make rec closure codes:[3] vars:[]
                v3 = make block tag:0 vars:[v1, v0, v2]
                set g12 = v3
                Exit: jump 2

                TOS: a1, a2, ..
                Stack: [
                ]
                Final acc: ()
            "#]],
        );

        check(
            vec![
                Acc(0),
                GetField(1),
                Push,
                Acc(1),
                GetField(0),
                Push,
                Acc(1),
                Push,
                Acc(2),
                Prim(Primitive::MulFloat),
                Push,
                Acc(1),
                Push,
                Acc(2),
                Prim(Primitive::MulFloat),
                Prim(Primitive::AddFloat),
                Prim(Primitive::SqrtFloat),
                Return(3),
            ],
            BlockExit::Return,
            expect![[r#"
                Block:
                v0 = a1[1]
                v1 = a1[0]
                v2 = mul.f v0 v0
                v3 = mul.f v1 v1
                v4 = add.f v3 v2
                v5 = sqrt.f v4
                Exit: return v5

                TOS: a2, a3, ..
                Stack: [
                ]
                Final acc: v5
            "#]],
        );

        // Monster case - pervasives
        check(
            vec![
                GetGlobal(89),
                Push,
                GetGlobal(89),
                GetField(32),
                MakeBlock(1, 0),
                Pop(1),
                Push,
                GetGlobal(100),
                GetField(0),
                Push,
                GetGlobal(100),
                GetField(0),
                Pop(1),
                Apply1,
                Push,
                Closure(407, 0),
                Push,
                Acc(0),
                MakeBlock(1, 0),
                Pop(1),
                Push,
                GetGlobal(100),
                GetField(0),
                Push,
                GetGlobal(100),
                GetField(0),
                Pop(1),
                Apply1,
                Push,
                Closure(408, 0),
                Push,
                Closure(409, 0),
                Push,
                Const(0),
                Push,
                Acc(2),
                Apply1,
                Const(0),
                Push,
                Acc(1),
                Apply1,
                Push,
                Acc(1),
                Push,
                Acc(3),
                Push,
                Acc(5),
                Push,
                Acc(7),
                MakeBlock(4, 0),
                Pop(5),
                SetGlobal(311),
                Const(0),
                Push,
                GetGlobal(45),
                GetField(102),
                Apply1,
                MakeBlock(0, 0),
                SetGlobal(312),
                Stop,
            ],
            BlockExit::Stop,
            expect![[r#"
                Block:
                v0 = g89
                v1 = g89
                v2 = v1[32]
                v3 = make block tag:0 vars:[v2]
                v4 = g100
                v5 = v4[0]
                v6 = g100
                v7 = v6[0]
                v8 = apply v7 [v3]
                v9 = make closure code:407 vars:[]
                v10 = make block tag:0 vars:[v9]
                v11 = g100
                v12 = v11[0]
                v13 = g100
                v14 = v13[0]
                v15 = apply v14 [v10]
                v16 = make closure code:408 vars:[]
                v17 = make closure code:409 vars:[]
                v18 = apply v16 [0]
                v19 = apply v17 [0]
                v20 = make block tag:0 vars:[v8, v15, v16, v17]
                set g311 = v20
                v21 = g45
                v22 = v21[102]
                v23 = apply v22 [0]
                v24 = make block tag:0 vars:[]
                set g312 = v24
                Exit: stop

                TOS: a1, a2, ..
                Stack: [
                ]
                Final acc: ()
            "#]],
        );
    }
}
