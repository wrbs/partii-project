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

    #[error("Deserializing error: '{0}'")]
    DeserializingError(&'static str),
}
