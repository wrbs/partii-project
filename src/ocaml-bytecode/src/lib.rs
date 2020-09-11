use std::fs::File;

mod bytecode;
mod error;
mod instructions;
mod opcodes;
mod primitives;

pub mod trailer;

pub use error::ParseFileError;
pub use instructions::*;
pub use opcodes::*;

pub struct BytecodeFile {
    pub trailer: trailer::Trailer,
    pub primitives: Vec<String>,
}

pub fn parse_bytecode_file(f: &mut File) -> Result<BytecodeFile, ParseFileError> {
    let trailer = trailer::parse_trailer(f)?;
    let _code = bytecode::parse_bytecode(f, &trailer)?;
    let primitives = primitives::parse_primitives(f, &trailer)?;

    Ok(BytecodeFile {
        trailer,
        primitives,
    })
}
