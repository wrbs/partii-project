use crate::bytecode_files::{ParseFileError, Trailer};
use byteorder::{LittleEndian, ReadBytesExt};
use ocaml_jit_shared::{
    BytecodeRelativeOffset, Instruction, InstructionIterator, Primitive, PrimitiveLookup,
};
use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;

const CODE_SECTION: &str = "CODE";

#[allow(clippy::type_complexity)]
pub fn parse_bytecode(
    f: &mut File,
    trailer: &Trailer,
    primitives: &[String],
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, ParseFileError> {
    let prims = Prims(primitives);

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
    let instrs: Result<Vec<_>, _> =
        InstructionIterator::new(words.iter().copied(), prims).collect();
    Ok(instrs?)
}

struct Prims<'a>(&'a [String]);

impl<'a> PrimitiveLookup for Prims<'a> {
    fn get_primitive(&self, primitive_id: u32) -> Option<Primitive> {
        let primitive_name = self.0.get(primitive_id as usize)?.as_str();
        Primitive::from_str(primitive_name).ok()
    }
}
