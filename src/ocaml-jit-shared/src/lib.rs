mod instructions;
mod opcodes;
mod trace;

pub use instructions::*;
pub use opcodes::*;
pub use trace::*;

use serde::{Deserialize, Serialize};
use std::fmt;

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
