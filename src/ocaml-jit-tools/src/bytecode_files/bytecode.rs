use crate::bytecode_files::{ParseFileError, Trailer};
use byteorder::{LittleEndian, ReadBytesExt};
use ocaml_jit_shared::parse_instructions;
use std::fs::File;
use std::io::BufReader;

const CODE_SECTION: &str = "CODE";

pub fn parse_bytecode(f: &mut File, trailer: &Trailer) -> Result<(), ParseFileError> {
    /*let section = trailer.find_required_section(CODE_SECTION)?;

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

        let parsed = parse_instructions(words.iter().copied()).unwrap();

        // Print it out in the format
        // original location: instructions, for, that, location

        let mut first_line = true;
        let mut first_instruction_on_line = true;
        for (index, instruction) in parsed.instructions.iter().enumerate() {
            if let Some(original_location) = parsed.labels.get_by_right(&index) {
                if first_line {
                    first_line = false;
                } else {
                    println!();
                }

                print!("{}: ", original_location);

                first_instruction_on_line = true;
            }

            if first_instruction_on_line {
                first_instruction_on_line = false;
            } else {
                print!(", ");
            }

            print!("{:?}", instruction);
        }

        if !first_line {
            println!();
        }
    */
    Ok(())
}
