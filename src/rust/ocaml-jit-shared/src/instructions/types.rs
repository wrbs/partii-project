use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct BytecodeRelativeOffset(pub usize);

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
        match self {
            // Cases with labels
            Instruction::LabelDef(l) => Instruction::LabelDef(f(l)),
            Instruction::PushRetAddr(l) => Instruction::PushRetAddr(f(l)),
            Instruction::Closure(l, x) => Instruction::Closure(f(l), *x),
            Instruction::ClosureRec(ls, x) => {
                Instruction::ClosureRec(ls.iter().map(f).collect(), *x)
            }
            Instruction::Branch(l) => Instruction::Branch(f(l)),
            Instruction::BranchIf(l) => Instruction::BranchIf(f(l)),
            Instruction::BranchIfNot(l) => Instruction::BranchIfNot(f(l)),
            Instruction::BranchCmp(cmp, v, l) => Instruction::BranchCmp(*cmp, *v, f(l)),
            Instruction::Switch(l1s, l2s) => {
                let l1s_mapped = l1s.iter().map(&mut f).collect();
                let l2s_mapped = l2s.iter().map(&mut f).collect();
                Instruction::Switch(l1s_mapped, l2s_mapped)
            }
            Instruction::PushTrap(l) => Instruction::PushTrap(f(l)),

            // Other cases
            Instruction::Acc(x) => Instruction::Acc(*x),
            Instruction::EnvAcc(x) => Instruction::EnvAcc(*x),
            Instruction::Push => Instruction::Push,
            Instruction::Pop(x) => Instruction::Pop(*x),
            Instruction::Assign(x) => Instruction::Assign(*x),
            Instruction::Apply1 => Instruction::Apply1,
            Instruction::Apply2 => Instruction::Apply2,
            Instruction::Apply3 => Instruction::Apply3,
            Instruction::Apply(x) => Instruction::Apply(*x),
            Instruction::ApplyTerm(x, y) => Instruction::ApplyTerm(*x, *y),
            Instruction::Return(x) => Instruction::Return(*x),
            Instruction::Restart => Instruction::Restart,
            Instruction::Grab(x) => Instruction::Grab(*x),
            Instruction::OffsetClosure(x) => Instruction::OffsetClosure(*x),
            Instruction::GetGlobal(x) => Instruction::GetGlobal(*x),
            Instruction::SetGlobal(x) => Instruction::SetGlobal(*x),
            Instruction::Const(x) => Instruction::Const(*x),
            Instruction::MakeBlock(x, y) => Instruction::MakeBlock(*x, *y),
            Instruction::MakeFloatBlock(x) => Instruction::MakeFloatBlock(*x),
            Instruction::GetField(x) => Instruction::GetField(*x),
            Instruction::SetField(x) => Instruction::SetField(*x),
            Instruction::GetFloatField(x) => Instruction::GetFloatField(*x),
            Instruction::SetFloatField(x) => Instruction::SetFloatField(*x),
            Instruction::VecTLength => Instruction::VecTLength,
            Instruction::GetVecTItem => Instruction::GetVecTItem,
            Instruction::SetVecTItem => Instruction::SetVecTItem,
            Instruction::GetBytesChar => Instruction::GetBytesChar,
            Instruction::SetBytesChar => Instruction::SetBytesChar,
            Instruction::BoolNot => Instruction::BoolNot,
            Instruction::PopTrap => Instruction::PopTrap,
            Instruction::Raise(x) => Instruction::Raise(*x),
            Instruction::CheckSignals => Instruction::CheckSignals,
            Instruction::CCall1(x) => Instruction::CCall1(*x),
            Instruction::CCall2(x) => Instruction::CCall2(*x),
            Instruction::CCall3(x) => Instruction::CCall3(*x),
            Instruction::CCall4(x) => Instruction::CCall4(*x),
            Instruction::CCall5(x) => Instruction::CCall5(*x),
            Instruction::CCallN(x, y) => Instruction::CCallN(*x, *y),
            Instruction::ArithInt(x) => Instruction::ArithInt(*x),
            Instruction::NegInt => Instruction::NegInt,
            Instruction::IntCmp(x) => Instruction::IntCmp(*x),
            Instruction::OffsetInt(x) => Instruction::OffsetInt(*x),
            Instruction::OffsetRef(x) => Instruction::OffsetRef(*x),
            Instruction::IsInt => Instruction::IsInt,
            Instruction::GetMethod => Instruction::GetMethod,
            Instruction::SetupForPubMet(x) => Instruction::SetupForPubMet(*x),
            Instruction::GetDynMet => Instruction::GetDynMet,
            Instruction::Stop => Instruction::Stop,
            Instruction::Break => Instruction::Break,
            Instruction::Event => Instruction::Event,
        }
    }

    pub fn modify_labels<F: FnMut(&mut L1)>(&mut self, mut f: F) {
        match self {
            // Cases with labels
            Instruction::LabelDef(l) => f(l),
            Instruction::PushRetAddr(l) => f(l),
            Instruction::Closure(l, _) => f(l),
            Instruction::ClosureRec(ls, _) => {
                ls.iter_mut().for_each(&mut f);
            }
            Instruction::Branch(l) => f(l),
            Instruction::BranchIf(l) => f(l),
            Instruction::BranchIfNot(l) => f(l),
            Instruction::BranchCmp(_, _, l) => f(l),
            Instruction::Switch(l1s, l2s) => {
                l1s.iter_mut().for_each(&mut f);
                l2s.iter_mut().for_each(&mut f);
            }
            Instruction::PushTrap(l) => f(l),

            _ => (),
        }
    }

    pub fn visit_labels<F: FnMut(&L1)>(&self, mut f: F) {
        match self {
            // Cases with labels
            Instruction::LabelDef(l) => f(l),
            Instruction::PushRetAddr(l) => f(l),
            Instruction::Closure(l, _) => f(l),
            Instruction::ClosureRec(ls, _) => {
                ls.iter().for_each(f);
            }
            Instruction::Branch(l) => f(l),
            Instruction::BranchIf(l) => f(l),
            Instruction::BranchIfNot(l) => f(l),
            Instruction::BranchCmp(_, _, l) => f(l),
            Instruction::Switch(l1s, l2s) => {
                l1s.iter().for_each(&mut f);
                l2s.iter().for_each(&mut f);
            }
            Instruction::PushTrap(l) => f(l),

            _ => (),
        }
    }
}
