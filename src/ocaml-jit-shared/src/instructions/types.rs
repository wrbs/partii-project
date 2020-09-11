use std::fmt::Debug;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ArithOp {
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Lsl,
    Lsr,
    Asr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RaiseKind {
    Regular,
    ReRaise,
    NoTrace,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Comp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    ULt,
    UGe,
}

// Instructions are generic over a few parameters
// L: the type of labels
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Instruction<Label = usize, Primitive = usize> {
    Acc(u32),
    EnvAcc(u32),
    Push,
    Pop(u32),
    Assign(u32),
    PushRetAddr(Label),
    Apply(u32),
    ApplyTerm(u32, u32),
    Return(u32),
    Restart,
    Grab(u32),
    Closure(Label, u32),
    ClosureRec(Vec<Label>, u32),
    OffsetClosure(i32),
    GetGlobal(u32),
    SetGlobal(u32),
    Const(i32),
    // we use MakeBlock(0, tag) for Atom()
    MakeBlock(u32, u8),
    MakeFloatBlock(u32),
    GetField(u32),
    SetField(u32),
    GetFloatField(u32),
    SetFloatField(u32),
    VecTLength,
    GetVecTItem,
    SetVecTItem,
    GetStringChar,
    GetBytesChar,
    SetBytesChar,
    Branch(Label),
    BranchIf(Label),
    BranchIfNot(Label),
    Switch(Vec<Label>, Vec<Label>),
    BoolNot,
    PushTrap(Label),
    PopTrap,
    Raise(RaiseKind),
    CheckSignals,
    CCall(u32, Primitive),
    ArithInt(ArithOp),
    IntCmp(Comp),
    BranchCmp(Comp, i32, Label),
    OffsetInt(i32),
    OffsetRef(i32),
    IsInt,
    GetMethod,
    GetPubMet(i32, u32),
    GetDynMet,
    Stop,
    Break,
    Event,
}
