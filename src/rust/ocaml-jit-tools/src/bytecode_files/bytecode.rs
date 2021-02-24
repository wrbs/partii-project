use std::io::Cursor;
use std::str::FromStr;

use byteorder::{LittleEndian, ReadBytesExt};

use ocaml_jit_shared::{
    BytecodeRelativeOffset, Instruction, InstructionIterator, Primitive, PrimitiveLookup,
};

use crate::bytecode_files::ParseFileError;

#[allow(clippy::type_complexity)]
pub fn parse_bytecode(
    code: &[u8],
    primitives: &[String],
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, ParseFileError> {
    let prims = Prims(primitives);
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
