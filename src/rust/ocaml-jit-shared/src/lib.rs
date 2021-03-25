use std::fmt;

use serde::{Deserialize, Serialize};

pub use instruction_trace::*;
pub use instructions::*;
pub use opcodes::*;

pub mod basic_blocks;
pub mod call_trace;
pub mod cranelift_compiler;
mod instruction_trace;
mod instructions;
mod opcodes;

pub use anyhow;
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
