// This module contains a representation of the contents of a closure as a CFG of basic blocks
mod conversion;
mod types;

pub use conversion::parse_to_basic_blocks;
pub use types::*;
