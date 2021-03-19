use crate::{ArithOp, Comp, RaiseKind};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicClosure {
    pub arity: usize,
    pub blocks: Vec<BasicBlock>,
    pub used_closures: Vec<usize>,
    pub max_stack_size: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub enum BasicBlockType {
    Normal,
    First,
    Trap,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicBlock {
    pub block_id: usize,
    pub predecessors: Vec<usize>,
    pub block_type: BasicBlockType,
    pub instructions: Vec<BasicBlockInstruction>,
    pub exit: BasicBlockExit,
    pub start_stack_size: u32,
    pub end_stack_size: u32,

    // When traversing the blocks in their ordering, which blocks
    // is this block the last predecessor of - used to seal blocks
    // in the SSA conversion scheme
    pub sealed_blocks: Vec<usize>,
}

impl BasicBlock {
    pub fn has_back_edges(&self) -> bool {
        self.predecessors
            .iter()
            .any(|other_id| *other_id >= self.block_id)
    }
}

// Instructions are generic over a few parameters
// L: the type of labels
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum BasicBlockInstruction {
    // Stack manipulation
    Acc(u32),
    EnvAcc(u32),
    Push,
    Pop(u32),
    Assign(u32),

    // Function calling
    Apply1,
    Apply2,
    Apply3,
    PushRetAddr,
    Apply(u32),

    // Allocation
    Closure(usize, u32),
    ClosureRec(Vec<usize>, u32),
    MakeBlock(u32, u8),
    MakeFloatBlock(u32),

    // Memory access
    OffsetClosure(i32),
    GetGlobal(u32),
    SetGlobal(u32),
    GetField(u32),
    SetField(u32),
    GetFloatField(u32),
    SetFloatField(u32),
    GetVecTItem,
    SetVecTItem,
    GetBytesChar,
    SetBytesChar,
    OffsetRef(i32),

    // Arith
    Const(i32),
    BoolNot,
    NegInt,
    ArithInt(ArithOp),
    IsInt,
    IntCmp(Comp),
    OffsetInt(i32),

    // Primitives
    CCall1(u32),
    CCall2(u32),
    CCall3(u32),
    CCall4(u32),
    CCall5(u32),
    CCallN { nargs: u32, id: u32 },

    // Other
    VecTLength,
    CheckSignals,
    PopTrap,
    GetMethod,
    SetupForPubMet(i32),
    GetDynMet,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum BasicBlockExit {
    // Branches
    Branch(usize),
    BranchIf {
        then_block: usize,
        else_block: usize,
    },
    BranchCmp {
        // they have different semantics with what they do to the accu
        cmp: Comp,
        constant: i32,
        then_block: usize,
        else_block: usize,
    },
    Switch {
        ints: Vec<usize>,
        tags: Vec<usize>,
    },

    // Traps
    PushTrap {
        normal: usize,
        trap: usize,
    },

    // Exits
    Return(u32),
    TailCall {
        args: u32,
        to_pop: u32,
    },
    Raise(RaiseKind),
    Stop,
}

impl BasicBlockExit {
    pub fn modify_block_labels<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut usize),
    {
        use BasicBlockExit::*;
        match self {
            Branch(l) => f(l),
            BranchIf {
                then_block,
                else_block,
            } => {
                f(then_block);
                f(else_block);
            }
            BranchCmp {
                then_block,
                else_block,
                ..
            } => {
                f(then_block);
                f(else_block);
            }
            Switch { ints, tags } => {
                ints.iter_mut().for_each(&mut f);
                tags.iter_mut().for_each(&mut f);
            }
            PushTrap { normal, trap } => {
                f(normal);
                f(trap);
            }
            Return(_) => {}
            TailCall { .. } => {}
            Raise(_) => {}
            Stop => {}
        }
    }
}