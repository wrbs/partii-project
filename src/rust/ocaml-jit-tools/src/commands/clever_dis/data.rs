// Data model for parsed instructions

use crate::bytecode_files::{MLValue, MLValueBlocks};
use ocaml_jit_shared::Instruction;

#[derive(Debug)]
pub struct Program {
    pub closures: Vec<Closure>,
    pub global_data_blocks: MLValueBlocks,
    pub globals: Vec<GlobalTableEntry>,
    pub primitives: Vec<String>,
}

#[derive(Debug)]
pub struct Closure {
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Block {
    pub instructions: Vec<Instruction<usize>>,
    pub closures: Vec<usize>,
    pub traps: Vec<usize>,
    pub exit: BlockExit,
}

#[derive(Debug)]
pub enum BlockExit {
    UnconditionalJump(usize),
    ConditionalJump(usize, usize),
    Switch(Vec<usize>),
    TailCall,
    Return,
    Raise,
    Stop,
}

#[derive(Debug)]
pub enum GlobalTableEntry {
    Constant(MLValue),
    Global(String),
}
