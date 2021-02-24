use std::fmt::{Display, Formatter};

use ocaml_jit_shared::{ArithOp, Comp, RaiseKind};

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
    Computed(usize, usize),
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
            SSAVar::Arg(i) => write!(f, "<arg:{}>", i),
            SSAVar::Env(i) => write!(f, "<env:{}>", i),
            SSAVar::Computed(block_num, i) => write!(f, "<{}_{}>", block_num, i),
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
            SSAExpr::GetGlobal(n) => write!(f, "global {}", n)?,
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
    Assign(usize, usize, SSAExpr),
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
            SSAStatement::Assign(block_num, i, expr) => {
                write!(f, "<{}_{}> = {}", block_num, i, expr)?;
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
                write!(f, "set global {} = {}", n, v)?;
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

fn display_array<T: Display>(f: &mut Formatter, array: &[T]) -> std::fmt::Result {
    const MAX_ON_LINE: usize = 8;

    if array.len() > MAX_ON_LINE {
        write!(f, "[")?;
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
