// Data model for parsed instructions

use crate::bytecode_files::debug_events::{DebugSpan, Ident};
use crate::bytecode_files::{MLValue, MLValueBlocks};
use ocaml_jit_shared::Instruction;
use std::process::id;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct Program {
    pub closures: Vec<Closure>,
    pub global_data_blocks: MLValueBlocks,
    pub globals: Vec<GlobalTableEntry>,
    pub primitives: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub module: Rc<String>,
    pub def_name: Rc<String>,
    pub filename: Rc<String>,
    pub heap_env: Vec<(i64, Ident)>,
    pub rec_env: Vec<(i64, Ident)>,
}
// debug position info goes here

#[derive(Debug, Clone)]
pub struct Closure {
    pub blocks: Vec<Block>,
    pub position: Option<PositionInfo>,
}

fn lookup_ident(table: &[(i64, Ident)], wanted_id: i64) -> Option<&Ident> {
    table.iter().find_map(
        |(id, ident)| {
            if *id == wanted_id {
                Some(ident)
            } else {
                None
            }
        },
    )
}

impl Closure {
    pub fn lookup_heap_ident(&self, wanted_id: usize) -> Option<&Ident> {
        self.position
            .as_ref()
            .and_then(|pi| lookup_ident(&pi.heap_env, wanted_id as i64))
    }

    pub fn lookup_closure_ident(&self, wanted_offset: i64) -> Option<&Ident> {
        self.position
            .as_ref()
            .and_then(|pi| lookup_ident(&pi.rec_env, wanted_offset))
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub instructions: Vec<Instruction<usize>>,
    pub closures: Vec<usize>,
    pub traps: Vec<usize>,
    pub exit: BlockExit,
}

#[derive(Debug, Clone)]
pub enum BlockExit {
    UnconditionalJump(usize),
    ConditionalJump(usize, usize),
    Switch {
        ints: Vec<usize>,
        blocks: Vec<usize>,
    },
    TailCall,
    Return,
    Raise,
    Stop,
}

#[derive(Debug, Clone)]
pub enum GlobalTableEntry {
    Constant(MLValue),
    Global(String),
}
