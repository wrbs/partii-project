mod bytecode;
mod error;
mod primitives;
pub mod trailer;

pub use error::ParseFileError;
use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction};
use std::fs::File;
pub use trailer::Trailer;

pub struct BytecodeFile {
    pub trailer: Trailer,
    pub primitives: Vec<String>,
    pub instructions: Vec<Instruction<BytecodeRelativeOffset>>,
}

pub fn parse_bytecode_file(f: &mut File) -> Result<BytecodeFile, ParseFileError> {
    let trailer = trailer::parse_trailer(f)?;
    let primitives = primitives::parse_primitives(f, &trailer)?;
    let instructions = bytecode::parse_bytecode(f, &trailer, &primitives)?;

    Ok(BytecodeFile {
        trailer,
        primitives,
        instructions,
    })
}
