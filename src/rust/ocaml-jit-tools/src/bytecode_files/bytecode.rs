use byteorder::{LittleEndian, ReadBytesExt};

use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction, InstructionIterator};

use crate::bytecode_files::{trailer::CODE_SECTION, ParseFileError, Trailer};
use std::fs::File;

pub fn read_code_section(f: &mut File, trailer: &Trailer) -> Result<Vec<i32>, ParseFileError> {
    let code_section = trailer.find_required_section(CODE_SECTION)?;
    let section_length = code_section.length;
    if section_length % 4 != 0 {
        return Err(ParseFileError::BadSize(
            "CODE section is not a multiple of 4",
        ));
    }

    let mut code = code_section.read_section(f)?;
    let mut words = Vec::new();

    for _ in 0..(section_length / 4) {
        words.push(code.read_i32::<LittleEndian>()?);
    }

    Ok(words)
}

#[allow(clippy::type_complexity)]
pub fn parse_bytecode(
    code: &[i32],
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, ParseFileError> {
    let instrs: Result<Vec<_>, _> = InstructionIterator::new(code.iter().copied()).collect();
    Ok(instrs?)
}
