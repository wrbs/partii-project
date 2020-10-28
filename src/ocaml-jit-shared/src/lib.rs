mod instructions;
mod opcodes;
mod trace;

pub use instructions::*;
pub use opcodes::*;
pub use trace::*;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct BytecodeLocation {
    pub section_number: usize,
    pub offset: BytecodeRelativeOffset,
}
