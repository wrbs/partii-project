use std::fmt;

use serde::{Deserialize, Serialize};

pub use instructions::*;
pub use opcodes::*;
pub use trace::*;

pub mod basic_blocks;
pub mod cranelift_compiler;
mod instructions;
mod opcodes;
mod trace;

pub use cranelift;
pub use cranelift_codegen;
pub use cranelift_module;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct BytecodeLocation {
    pub section_number: usize,
    pub offset: BytecodeRelativeOffset,
}

impl fmt::Display for BytecodeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}; {}>", self.section_number, self.offset.0)
    }
}
