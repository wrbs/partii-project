mod bytecode;
mod error;
mod primitives;
pub mod trailer;

pub use error::ParseFileError;
use std::fs::File;
pub use trailer::Trailer;
use ocaml_jit_shared::Instruction;

pub struct BytecodeFile {
    pub trailer: Trailer,
    pub primitives: Vec<String>,
    pub instructions: Vec<(usize, Vec<Instruction<usize>>)>,
}

pub fn parse_bytecode_file(f: &mut File) -> Result<BytecodeFile, ParseFileError> {
    let trailer = trailer::parse_trailer(f)?;
    let instructions = bytecode::parse_bytecode(f, &trailer)?;
    let primitives = primitives::parse_primitives(f, &trailer)?;

    Ok(BytecodeFile {
        trailer,
        primitives,
        instructions
    })
}
