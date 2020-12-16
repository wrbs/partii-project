use ocaml_jit_shared::InstructionParseError;
use std::io;
use thiserror::Error;

// all the ways it can go wrong
#[derive(Error, Debug)]
pub enum ParseFileError {
    #[error("IO Error: {0}")]
    IO(#[from] io::Error),

    #[error("Wrong magic number")]
    WrongMagic,

    #[error("Bad sizes: {0}")]
    BadSize(&'static str),

    #[error("Section '{0}' not found")]
    SectionNotFound(&'static str),

    #[error("Invalid primitive formatting found")]
    BadPrimitiveFormatting,

    #[error("Error while parsing: {0}")]
    ParsingError(#[from] InstructionParseError),
}
