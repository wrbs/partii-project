mod parse;
mod types;

use crate::Opcode;

pub use parse::parse_instructions;
pub use types::{ArithOp, Comp, Instruction, RaiseKind};
