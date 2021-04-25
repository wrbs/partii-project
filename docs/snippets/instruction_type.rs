#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ArithOp {
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

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
pub enum RaiseKind {
    Regular,
    ReRaise,
    NoTrace,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
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
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum Instruction<L> {
    LabelDef(L),
    Acc(u32),
    EnvAcc(u32),
    Push,
    Pop(u32),
    Assign(u32),
    PushRetAddr(L),
    Apply1,
    Apply2,
    Apply3,
    Apply(u32),
    ApplyTerm(u32, u32),
    Return(u32),
    Restart,
    Grab(u32),
    Closure(L, u32),
    ClosureRec(Vec<L>, u32),
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
    GetBytesChar,
    SetBytesChar,
    Branch(L),
    BranchIf(L),
    BranchIfNot(L),
    Switch(Vec<L>, Vec<L>),
    BoolNot,
    PushTrap(L),
    PopTrap,
    Raise(RaiseKind),
    CheckSignals,
    CCall1(u32),
    CCall2(u32),
    CCall3(u32),
    CCall4(u32),
    CCall5(u32),
    CCallN(u32, u32),
    ArithInt(ArithOp),
    NegInt,
    IntCmp(Comp),
    BranchCmp(Comp, i32, L),
    OffsetInt(i32),
    OffsetRef(i32),
    IsInt,
    GetMethod,
    SetupForPubMet(i32),
    GetDynMet,
    Stop,
    Break,
    Event,
}

impl<L1> Instruction<L1> {
    pub fn map_labels<L2, F: FnMut(&L1) -> L2>(&self, mut f: F) -> Instruction<L2> {
        // ...
    }

    pub fn modify_labels<F: FnMut(&mut L1)>(&mut self, mut f: F) {
        // ...
    }

    pub fn visit_labels<F: FnMut(&L1)>(&self, mut f: F) {
        // ...
    }
}
