mod types;
mod parse;

use crate::Opcode;

pub use types::{Instruction, ArithOp, RaiseKind, Comp};
pub use parse::parse_instructions;

