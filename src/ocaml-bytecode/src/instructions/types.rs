use std::fmt::Debug;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ArithOp {
    Neg, Add, Sub, Mul, Div, Mod,
    And, Or,  Xor, Lsl, Lsr, Asr
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RaiseKind {
    Regular, ReRaise, NoTrace
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Comp {
    Eq, Ne, Lt, Le, Gt, Ge, ULt, UGe
}

// Instructions are generic over a few parameters
// L: the type of labels
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Instruction<Label=usize, Primitive=usize> {
    Acc(i32),
    EnvAcc(i32),
    Push,
    PushAcc(i32),
    PushEnvAcc(i32),
    Pop(i32),
    Assign(i32),
    PushRetAddr(Label),
    Apply(i32),
    ApplyTerm(i32, i32),
    Return(i32),
    Restart,
    Grab(i32),
    Closure(Label, i32),
    ClosureRec(Vec<Label>, i32),
    OffsetClosure(i32),
    PushOffsetClosure(i32),
    GetGlobal(i32),
    PushGetGlobal(i32),
    GetGlobalField(i32, i32),
    PushGetGlobalField(i32, i32),
    SetGlobal(i32),
    Const(i32),
    PushConst(i32),
    // we use MakeBlock(0, tag) for Atom()
    MakeBlock(i32, i32),
    MakeFloatBlock(i32),
    // but not PushAtom
    PushAtom(i32),
    GetField(i32),
    SetField(i32),
    GetFloatField(i32),
    SetFloatField(i32),
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
    CCall(i32, Primitive),
    ArithInt(ArithOp),
    IntCmp(Comp),
    BranchCmp(Comp, i32, Label),
    OffsetInt(i32),
    OffsetRef(i32),
    IsInt,
    GetMethod,
    GetPubMet(i32),
    GetDynMet,
    Stop,
    Break,
    Event,
}
