#[cfg(test)]
mod tests;

use crate::commands::clever_dis::data::{Block, BlockExit, Closure, Program};
use anyhow::{bail, ensure, Result};
use ocaml_jit_shared::{ArithOp, Comp, Instruction, Primitive, RaiseKind};
use std::cmp::max;
use std::env::args;
use std::fmt::{Binary, Display, Formatter};
use std::process::exit;
use strum_macros;

fn display_array<T: Display>(f: &mut Formatter, array: &[T]) -> std::fmt::Result {
    const MAX_ON_LINE: usize = 8;

    if array.len() > MAX_ON_LINE {
        write!(f, "[")?;
        let count = 0;
        for (count, v) in array.iter().enumerate() {
            if count % MAX_ON_LINE == 0 {
                write!(f, "\n   ")?;
            }
            write!(f, " {},", v)?;
        }

        write!(f, "\n]")?;
    } else {
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
    }

    Ok(())
}

#[derive(Debug)]
pub struct SSABlock {
    pub statements: Vec<SSAStatement>,
    pub exit: SSAExit,
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
    PrevStack(usize),
    PrevAcc,
    Arg(usize),
    Env(usize),
    Computed(usize),
    OffsetClosure(isize),
    Const(i32),
    Unit,
    Atom(u8),
    Special, // Traps and such like
    Junk,
}

impl Display for SSAVar {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAVar::PrevStack(i) => write!(f, "<prev:{}>", i),
            SSAVar::PrevAcc => write!(f, "<prev:acc>"),
            SSAVar::Arg(i) => write!(f, "a{}", i),
            SSAVar::Env(i) => write!(f, "<env:{}>", i),
            SSAVar::Computed(i) => write!(f, "v{}", i),
            SSAVar::OffsetClosure(i) => write!(f, "<closure:{}>", i),
            SSAVar::Const(i) => write!(f, "{}", i),
            SSAVar::Unit => write!(f, "<unit>"),
            SSAVar::Atom(tag) => write!(f, "<atom:{}>", tag),
            SSAVar::Special => write!(f, "<special>"),
            SSAVar::Junk => write!(f, "<junk>"),
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, strum_macros::Display)]
pub enum UnaryOp {
    #[strum(serialize = "neg int")]
    Neg,
    #[strum(serialize = "not")]
    BoolNot,
    #[strum(serialize = "is_int")]
    IsInt,
}

#[derive(Debug)]
pub enum SSAExpr {
    Apply(SSAVar, Vec<SSAVar>),
    GetGlobal(usize),
    GetField(SSAVar, SSAVar),
    GetFloatField(SSAVar, usize),
    GetBytesChar(SSAVar, SSAVar),
    GetVecTLength(SSAVar),
    ArithInt(ArithOp, SSAVar, SSAVar),
    UnaryOp(UnaryOp, SSAVar),
    IntCmp(Comp, SSAVar, SSAVar),
    UnaryFloat(UnaryFloatOp, SSAVar),
    BinaryFloat(BinaryFloatOp, SSAVar, SSAVar),
    MakeBlock {
        tag: u8,
        vars: Vec<SSAVar>,
    },
    MakeFloatBlock(Vec<SSAVar>),
    Closure {
        code: usize,
        vars: Vec<SSAVar>,
    },
    ClosureRec {
        codes: Vec<usize>,
        vars: Vec<SSAVar>,
    },
    ClosureRecInfix(SSAVar, usize),
    CCall {
        primitive_id: usize,
        vars: Vec<SSAVar>,
    },
    GetMethod(SSAVar, SSAVar),
    GetDynMet {
        tag: SSAVar,
        object: SSAVar,
    },
}

impl Display for SSAExpr {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAExpr::Apply(closure, args) => {
                write!(f, "apply {} ", closure)?;

                display_array(f, args)?;
            }
            SSAExpr::GetGlobal(n) => write!(f, "g{}", n)?,
            SSAExpr::GetField(v, i) => write!(f, "{}[{}]", v, i)?,
            SSAExpr::GetFloatField(v, i) => write!(f, "float {}[{}]", v, i)?,
            SSAExpr::GetBytesChar(v, i) => write!(f, "bytes {}[{}]", v, i)?,
            SSAExpr::GetVecTLength(v) => write!(f, "vec_t length {}", v)?,
            SSAExpr::ArithInt(op, a, b) => match op {
                ArithOp::Add => write!(f, "{} + {}", a, b)?,
                ArithOp::Sub => write!(f, "{} - {}", a, b)?,
                ArithOp::Mul => write!(f, "{} * {}", a, b)?,
                ArithOp::Div => write!(f, "{} / {}", a, b)?,
                ArithOp::Mod => write!(f, "{} % {}", a, b)?,
                ArithOp::And => write!(f, "{} & {}", a, b)?,
                ArithOp::Or => write!(f, "{} | {}", a, b)?,
                ArithOp::Xor => write!(f, "{} ^ {}", a, b)?,
                ArithOp::Lsl => write!(f, "{} << {}", a, b)?,
                ArithOp::Lsr => write!(f, "{} l>> {}", a, b)?,
                ArithOp::Asr => write!(f, "{} a>> {}", a, b)?,
            },
            SSAExpr::UnaryOp(op, v) => write!(f, "{} {}", op, v)?,
            SSAExpr::IntCmp(comp, a, b) => match comp {
                Comp::Eq => write!(f, "{} == {}", a, b)?,
                Comp::Ne => write!(f, "{} != {}", a, b)?,
                Comp::Lt => write!(f, "{} < {}", a, b)?,
                Comp::Le => write!(f, "{} <= {}", a, b)?,
                Comp::Gt => write!(f, "{} > {}", a, b)?,
                Comp::Ge => write!(f, "{} >= {}", a, b)?,
                Comp::ULt => write!(f, "{} u< {}", a, b)?,
                Comp::UGe => write!(f, "{} u>= {}", a, b)?,
            },
            SSAExpr::BinaryFloat(op, a, b) => {
                write!(f, "{} {} {}", op, a, b)?;
            }
            SSAExpr::UnaryFloat(op, x) => {
                write!(f, "{} {}", op, x)?;
            }
            SSAExpr::MakeBlock { tag, vars } => {
                write!(f, "make block tag:{} vars:", tag)?;
                display_array(f, vars)?;
            }
            SSAExpr::MakeFloatBlock(vars) => {
                write!(f, "make float block ")?;
                display_array(f, vars)?;
            }
            SSAExpr::Closure { code, vars } => {
                write!(f, "make closure code:{} vars:", code)?;
                display_array(f, vars)?;
            }
            SSAExpr::ClosureRec { codes, vars } => {
                write!(f, "make rec closure codes:")?;
                display_array(f, codes)?;
                write!(f, " vars:")?;
                display_array(f, vars)?;
            }
            SSAExpr::ClosureRecInfix(closure_rec, offset) => {
                write!(f, "rec closure infix {}[{}]", closure_rec, offset)?;
            }
            SSAExpr::CCall { primitive_id, vars } => {
                write!(f, "ccall {} ", primitive_id)?;
                display_array(f, vars)?;
            }
            SSAExpr::GetMethod(a, b) => {
                write!(f, "get method {} {}", a, b)?;
            }
            SSAExpr::GetDynMet { tag, object } => {
                write!(f, "get dynmet tag:{} object:{} ", tag, object)?;
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
    Grab(usize),
    SetGlobal(usize, SSAVar),
    SetField(SSAVar, SSAVar, SSAVar),
    SetFloatField(SSAVar, usize, SSAVar),
    SetBytesChar(SSAVar, SSAVar, SSAVar),
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
            SSAStatement::Grab(i) => {
                write!(f, "grab {}", i)?;
            }
            SSAStatement::SetGlobal(n, v) => {
                write!(f, "set g{} = {}", n, v)?;
            }
            SSAStatement::SetField(b, n, v) => {
                write!(f, "set {}[{}] = {}", b, n, v)?;
            }
            SSAStatement::SetFloatField(b, n, v) => {
                write!(f, "set float {}[{}] = {}", b, n, v)?;
            }
            SSAStatement::SetBytesChar(b, n, v) => {
                write!(f, "set bytes {}[{}] = {}", b, n, v)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum SSAExit {
    Stop(SSAVar),
    Jump(usize),
    JumpIf {
        var: SSAVar,
        if_true: usize,
        if_false: usize,
    },
    Switch {
        var: SSAVar,
        ints: Vec<usize>,
        blocks: Vec<usize>,
    },
    TailApply(SSAVar, Vec<SSAVar>),
    PushTrap {
        normal: usize,
        trap: usize,
    },
    Raise(RaiseKind, SSAVar),
    Return(SSAVar),
}

impl Display for SSAExit {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SSAExit::Stop(v) => {
                write!(f, "stop {}", v)?;
            }
            SSAExit::Jump(block) => {
                write!(f, "jump {}", block)?;
            }
            SSAExit::JumpIf {
                var,
                if_true,
                if_false,
            } => {
                write!(f, "jump_if {} t:{} f:{}", var, if_true, if_false)?;
            }
            SSAExit::Switch { var, ints, blocks } => {
                write!(f, "switch {} ints:", var)?;
                display_array(f, ints)?;
                write!(f, " blocks:")?;
                display_array(f, blocks)?;
            }
            SSAExit::TailApply(closure, args) => {
                write!(f, "tail_apply {} ", closure)?;

                display_array(f, args)?;
            }
            SSAExit::PushTrap { normal, trap } => {
                write!(f, "push trap normal:{} trap:{}", normal, trap)?;
            }
            SSAExit::Raise(RaiseKind::Regular, v) => {
                write!(f, "raise {}", v)?;
            }
            SSAExit::Raise(RaiseKind::NoTrace, v) => {
                write!(f, "raise_notrace {}", v)?;
            }
            SSAExit::Raise(RaiseKind::ReRaise, v) => {
                write!(f, "reraise {}", v)?;
            }
            SSAExit::Return(v) => {
                write!(f, "return {}", v)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct State {
    pub stack: Vec<SSAVar>,
    pub acc: SSAVar,
    pub stack_start: usize,
}

impl State {
    fn pick(&self, n: usize) -> SSAVar {
        if n < self.stack.len() {
            self.stack[self.stack.len() - 1 - n]
        } else {
            let arg_offset = n - self.stack.len();
            return SSAVar::PrevStack(self.stack_start + arg_offset);
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
            for i in 0..todo {
                tmp_stack.push(SSAVar::PrevStack(self.stack_start - i));
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
        writeln!(f, "Final acc: {}", self.acc)?;

        write!(f, "End stack: ..., <prev:{}> | ", self.stack_start)?;

        let mut first = true;

        for entry in &self.stack {
            if first {
                first = false
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{}", entry)?;
        }
        writeln!(f)?;

        writeln!(
            f,
            "Stack delta: -{}/+{}",
            self.stack_start,
            self.stack.len()
        )?;

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

pub fn translate_block(block: &Block, is_entry_block: bool) -> Result<(SSABlock, State)> {
    ensure!(block.instructions.len() > 0);
    let last_instr_idx = block.instructions.len() - 1;

    let mut vars_d = Vars::new();
    let mut state_d = State {
        stack: vec![],
        acc: SSAVar::PrevAcc,
        stack_start: 0,
    };
    let vars = &mut vars_d;
    let state = &mut state_d;

    let has_grab = if is_entry_block {
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

    Ok((
        SSABlock {
            statements: vars_d.statements,
            exit,
        },
        state_d,
    ))
}

fn process_body_instruction(
    state: &mut State,
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
            state.push(SSAVar::Special);
            state.push(SSAVar::Special);
            state.push(SSAVar::Special);
        }
        Instruction::Apply1 => {
            state.acc = vars.add_assignment(SSAExpr::Apply(state.acc, vec![state.pick(0)]));
            state.pop(1);
        }
        Instruction::Apply2 => {
            state.acc = vars.add_assignment(SSAExpr::Apply(
                state.acc,
                vec![state.pick(0), state.pick(1)],
            ));
            state.pop(2);
        }
        Instruction::Apply3 => {
            state.acc = vars.add_assignment(SSAExpr::Apply(
                state.acc,
                vec![state.pick(0), state.pick(1), state.pick(2)],
            ));
            state.pop(3);
        }
        Instruction::Grab(_) => bail!("Grabs should not appear in the body of blocks"),
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
            state.push(state.acc);

            for i in 1..locs.len() {
                state.push(vars.add_assignment(SSAExpr::ClosureRecInfix(state.acc, i)));
            }
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
        Instruction::MakeBlock(size, tag) => {
            let size = *size as usize;
            let tag = *tag;
            if size == 0 {
                state.acc = SSAVar::Atom(tag);
            } else {
                state.push(state.acc);

                state.acc = vars.add_assignment(SSAExpr::MakeBlock {
                    tag,
                    vars: (0..size).map(|i| state.pick(i)).collect(),
                });

                state.pop(size);
            }
        }
        Instruction::MakeFloatBlock(size) => {
            let size = *size as usize;
            state.push(state.acc);
            state.acc = vars.add_assignment(SSAExpr::MakeFloatBlock(
                (0..size).map(|i| state.pick(i)).collect(),
            ));
            state.pop(size);
        }
        Instruction::GetField(n) => {
            state.acc = vars.add_assignment(SSAExpr::GetField(state.acc, SSAVar::Const(*n as i32)));
        }
        Instruction::SetField(n) => {
            vars.add_statement(SSAStatement::SetField(
                state.acc,
                SSAVar::Const(*n as i32),
                state.pick(0),
            ));
            state.pop(1);
            state.acc = SSAVar::Unit;
        }
        Instruction::GetFloatField(n) => {
            state.acc = vars.add_assignment(SSAExpr::GetFloatField(state.acc, *n as usize));
        }
        Instruction::SetFloatField(n) => {
            vars.add_statement(SSAStatement::SetFloatField(
                state.acc,
                *n as usize,
                state.pick(0),
            ));
            state.pop(1);
            state.acc = SSAVar::Unit;
        }
        Instruction::VecTLength => {
            state.acc = vars.add_assignment(SSAExpr::GetVecTLength(state.acc));
        }
        Instruction::GetVecTItem => {
            state.acc = vars.add_assignment(SSAExpr::GetField(state.acc, state.pick(0)));
            state.pop(1);
        }
        Instruction::SetVecTItem => {
            vars.add_statement(SSAStatement::SetField(
                state.acc,
                state.pick(0),
                state.pick(1),
            ));
            state.pop(2);
            state.acc = SSAVar::Unit;
        }
        Instruction::GetBytesChar => {
            state.acc = vars.add_assignment(SSAExpr::GetBytesChar(state.acc, state.pick(0)));
            state.pop(1);
        }
        Instruction::SetBytesChar => {
            vars.add_statement(SSAStatement::SetBytesChar(
                state.acc,
                state.pick(0),
                state.pick(1),
            ));
            state.pop(2);
            state.acc = SSAVar::Unit;
        }
        Instruction::BoolNot => {
            state.acc = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::BoolNot, state.acc))
        }
        Instruction::PopTrap => {
            vars.add_statement(SSAStatement::PopTrap);
            state.pop(4);
        }
        Instruction::CheckSignals => {
            vars.add_statement(SSAStatement::CheckSignals);
            state.acc = SSAVar::Junk;
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
            state.acc = vars.add_assignment(SSAExpr::ArithInt(*op, state.acc, state.pick(0)));
            state.pop(1);
        }
        Instruction::NegInt => {
            state.acc = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::Neg, state.acc))
        }
        Instruction::IntCmp(comp) => {
            state.acc = vars.add_assignment(SSAExpr::IntCmp(*comp, state.acc, state.pick(0)));
            state.pop(1);
        }
        Instruction::OffsetInt(n) => {
            state.acc = vars.add_assignment(SSAExpr::ArithInt(
                ArithOp::Add,
                state.acc,
                SSAVar::Const(*n),
            ));
        }
        Instruction::OffsetRef(n) => {
            // Todo investigate whether special casing helps avoid a caml_modify
            let a = vars.add_assignment(SSAExpr::GetField(state.acc, SSAVar::Const(0)));
            let b = vars.add_assignment(SSAExpr::ArithInt(ArithOp::Add, a, SSAVar::Const(*n)));
            vars.add_statement(SSAStatement::SetField(state.acc, SSAVar::Const(0), b));
            state.acc = SSAVar::Unit;
        }
        Instruction::IsInt => {
            state.acc = vars.add_assignment(SSAExpr::UnaryOp(UnaryOp::IsInt, state.acc))
        }
        Instruction::GetMethod => {
            state.acc = vars.add_assignment(SSAExpr::GetMethod(state.pick(0), state.acc));
            state.pop(1);
        }
        Instruction::SetupForPubMet(n) => {
            state.push(state.acc);
            state.acc = SSAVar::Const(*n);
        }
        Instruction::GetDynMet => {
            state.acc = vars.add_assignment(SSAExpr::GetDynMet {
                tag: state.acc,
                object: state.pick(0),
            });
            state.pop(1);
        }
        Instruction::Break | Instruction::Event => (),
    }

    Ok(())
}

fn process_final_instruction(
    state: &mut State,
    vars: &mut Vars,
    instr: &Instruction<usize>,
    exit: &BlockExit,
) -> Result<SSAExit> {
    let exit = match (instr, exit) {
        (Instruction::Stop, BlockExit::Stop) => SSAExit::Stop(state.acc),
        (Instruction::Branch(n2), BlockExit::UnconditionalJump(n1)) => {
            ensure!(n1 == n2);
            SSAExit::Jump(*n1)
        }
        (Instruction::Return(count), BlockExit::Return) => {
            state.pop(*count as usize);
            SSAExit::Return(state.acc)
        }
        (Instruction::BranchIf(to1), BlockExit::ConditionalJump(ift, iff)) => {
            ensure!(to1 == ift);
            SSAExit::JumpIf {
                var: state.acc,
                if_true: *ift,
                if_false: *iff,
            }
        }
        (Instruction::BranchIfNot(to1), BlockExit::ConditionalJump(iff, ift)) => {
            ensure!(to1 == iff);
            SSAExit::JumpIf {
                var: state.acc,
                if_true: *ift,
                if_false: *iff,
            }
        }
        (Instruction::BranchCmp(comp, compare, ift1), BlockExit::ConditionalJump(ift2, iff)) => {
            ensure!(ift1 == ift2);
            let v = vars.add_assignment(SSAExpr::IntCmp(*comp, SSAVar::Const(*compare), state.acc));
            SSAExit::JumpIf {
                var: v,
                if_true: *ift1,
                if_false: *iff,
            }
        }
        (Instruction::Raise(kind), BlockExit::Raise) => SSAExit::Raise(*kind, state.acc),
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

            state.acc = vars.add_assignment(SSAExpr::Apply(state.acc, passed_vars));
            SSAExit::Jump(retloc)
        }
        (Instruction::ApplyTerm(nargs, slotsize), BlockExit::TailCall) => {
            let nargs = *nargs as usize;
            let slotsize = *slotsize as usize;
            let vars = (0..nargs).map(|i| state.pick(i)).collect();
            state.pop(slotsize);
            SSAExit::TailApply(state.acc, vars)
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
                var: state.acc,
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
