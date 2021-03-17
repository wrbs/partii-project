use std::io::Cursor;
use std::str::FromStr;

use byteorder::{LittleEndian, ReadBytesExt};

use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction, InstructionIterator};

use crate::bytecode_files::ParseFileError;

#[allow(clippy::type_complexity)]
pub fn parse_bytecode(
    code: &[u8],
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, ParseFileError> {
    if code.len() % 4 != 0 {
        return Err(ParseFileError::BadSize(
            "CODE section is not a multiple of 4",
        ));
    }

    let mut f = Cursor::new(code);
    let mut words = Vec::new();

    for _ in 0..(code.len() / 4) {
        words.push(f.read_i32::<LittleEndian>()?);
    }
    let instrs: Result<Vec<_>, _> = InstructionIterator::new(words.iter().copied()).collect();
    Ok(instrs?)
}
