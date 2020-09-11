use crate::bytecode_files::{ParseFileError, Trailer};
use byteorder::{LittleEndian, ReadBytesExt};
use ocaml_jit_shared::parse_instructions;
use std::fs::File;
use std::io::BufReader;

const CODE_SECTION: &str = "CODE";

pub fn parse_bytecode(f: &mut File, trailer: &Trailer) -> Result<Vec<()>, ParseFileError> {
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

    let instructions = parse_instructions(words.iter().copied()).unwrap();

    for (loc, instruction) in instructions {
        println!("{:8}: {:?}", loc, instruction);
    }

    Ok(vec![])
}
