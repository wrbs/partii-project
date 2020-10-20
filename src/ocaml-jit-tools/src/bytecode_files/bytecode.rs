use crate::bytecode_files::{ParseFileError, Trailer};
use byteorder::{LittleEndian, ReadBytesExt};
use ocaml_jit_shared::{parse_instructions, Instruction};
use std::fs::File;
use std::io::BufReader;

const CODE_SECTION: &str = "CODE";

#[allow(clippy::type_complexity)]
pub fn parse_bytecode(
    f: &mut File,
    trailer: &Trailer,
) -> Result<Vec<(usize, Vec<Instruction<usize>>)>, ParseFileError> {
    let section = trailer.find_required_section(CODE_SECTION)?;

    if section.length % 4 != 0 {
        return Err(ParseFileError::BadSize(
            "CODE section is not a multiple of 4",
        ));
    }

    let mut section_read = BufReader::new(section.read_section(f)?);

    let mut words = Vec::new();

    for _ in 0..(section.length / 4) {
        words.push(section_read.read_i32::<LittleEndian>()?);
    }
    let parsed = parse_instructions(words.iter().copied(), words.len()).unwrap();

    let mut final_instructions = Vec::new();

    for (bytecode_location, maybe_parsed_offset) in parsed.lookup.iter().enumerate() {
        match maybe_parsed_offset {
            None => (),
            Some((start, num_instructions)) => {
                let start = *start;
                let end = start + *num_instructions;
                let contained_instructions = parsed.instructions[start..end].to_vec();
                final_instructions.push((bytecode_location, contained_instructions));
            }
        }
    }

    Ok(final_instructions)
}
