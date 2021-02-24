// Data model for parsed instructions

use std::rc::Rc;

use ocaml_jit_shared::Instruction;
use serde::{Deserialize, Serialize};

use crate::bytecode_files::debug_events::Ident;
use crate::bytecode_files::{MLValue, MLValueBlocks};

#[derive(Debug, Clone)]
pub struct Program {
    pub closures: Vec<Closure>,
    pub global_data_blocks: MLValueBlocks,
    pub globals: Vec<GlobalTableEntry>,
    pub primitives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub module: Rc<str>,
    pub def_name: Rc<str>,
    pub filename: Rc<str>,
    pub heap_env: Vec<(i64, Ident)>,
    pub rec_env: Vec<(i64, Ident)>,
}
// debug position info goes here

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub instructions: Vec<Instruction<usize>>,
    pub closures: Vec<usize>,
    pub exit: BlockExit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockExit {
    UnconditionalJump(usize),
    ConditionalJump(usize, usize),
    PushTrap {
        normal: usize,
        trap: usize,
    },
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
