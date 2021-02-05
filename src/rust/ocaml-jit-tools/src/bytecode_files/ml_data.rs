// A hacky implementation of the OCaml intern.c algorithm
// This is used for serialization in OCaml - specifically especially to store metadata inside of
// bytecode files

use anyhow::{bail, ensure, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::fmt::{Display, Formatter};
use std::io::Read;

#[derive(Debug, Clone)]
pub enum MLValue {
    Int(i64),
    Block { tag: u8, items: Vec<MLValue> },
    StringUtf8(String),
    StringBytes(Vec<u8>),
    Int32(i32),
    Int64(i64),
    Shared(usize),
    Double(f64),
}

impl Display for MLValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MLValue::Int(i) => write!(f, "{}", i),
            MLValue::StringUtf8(s) => write!(f, "{:?}", s),
            MLValue::StringBytes(vec) => write!(f, "StringBytes({:?})", vec),
            MLValue::Int32(i) => write!(f, "{}", i),
            MLValue::Int64(i) => write!(f, "{}", i),
            MLValue::Block { tag, items } => {
                write!(f, "{{{}:[", tag)?;
                let mut first = true;
                for item in items {
                    if first {
                        first = false;
                    } else {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }

                write!(f, "]}}")?;
                Ok(())
            }
            MLValue::Shared(offset) => write!(f, "Shared({})", offset),
            MLValue::Double(d) => write!(f, "{}", d),
        }
    }
}

// from intext.h
const INTEXT_MAGIC_NUMBER_SMALL: u32 = 0x8495A6BE;
const INTEXT_MAGIC_NUMBER_BIG: u32 = 0x8495A6BF;

const PREFIX_SMALL_BLOCK_END: u8 = 255;
const PREFIX_SMALL_BLOCK: u8 = 0x80;
const PREFIX_SMALL_INT_END: u8 = PREFIX_SMALL_BLOCK - 1;
const PREFIX_SMALL_INT: u8 = 0x40;
const PREFIX_SMALL_STRING_END: u8 = PREFIX_SMALL_INT - 1;
const PREFIX_SMALL_STRING: u8 = 0x20;
const CODE_INT8: u8 = 0x0;
const CODE_INT16: u8 = 0x1;
const CODE_INT32: u8 = 0x2;
const CODE_INT64: u8 = 0x3;
const CODE_SHARED8: u8 = 0x4;
const CODE_SHARED16: u8 = 0x5;
const CODE_SHARED32: u8 = 0x6;
const CODE_SHARED64: u8 = 0x14;
const CODE_BLOCK32: u8 = 0x8;
const CODE_BLOCK64: u8 = 0x13;
const CODE_STRING8: u8 = 0x9;
const CODE_STRING32: u8 = 0xA;
const CODE_STRING64: u8 = 0x15;
const CODE_DOUBLE_BIG: u8 = 0xB;
const CODE_DOUBLE_LITTLE: u8 = 0xC;
const CODE_DOUBLE_ARRAY8_BIG: u8 = 0xD;
const CODE_DOUBLE_ARRAY8_LITTLE: u8 = 0xE;
const CODE_DOUBLE_ARRAY32_BIG: u8 = 0xF;
const CODE_DOUBLE_ARRAY32_LITTLE: u8 = 0x7;
const CODE_DOUBLE_ARRAY64_BIG: u8 = 0x16;
const CODE_DOUBLE_ARRAY64_LITTLE: u8 = 0x17;
const CODE_CODEPOINTER: u8 = 0x10;
const CODE_INFIXPOINTER: u8 = 0x11;
const CODE_CUSTOM: u8 = 0x12 /* deprecated */;
const CODE_CUSTOM_LEN: u8 = 0x18;
const CODE_CUSTOM_FIXED: u8 = 0x19;

const OBJECT_TAG: u8 = 248;

struct Header {
    data_len: usize,
    num_objects: usize,
    whsize: usize,
}

pub fn input_value<R: Read>(f: &mut R) -> Result<MLValue> {
    let _ = parse_header(f)?;
    read_value(f)
}

fn parse_header<R: Read>(f: &mut R) -> Result<Header> {
    let magic = f.read_u32::<BigEndian>()?;

    let (data_len, num_objects, whsize) = match magic {
        INTEXT_MAGIC_NUMBER_BIG => {
            f.read_u32::<BigEndian>()?;
            let data_len = f.read_u64::<BigEndian>()? as usize;
            let num_objects = f.read_u64::<BigEndian>()? as usize;
            let whsize = f.read_u64::<BigEndian>()? as usize;

            (data_len, num_objects, whsize)
        }
        INTEXT_MAGIC_NUMBER_SMALL => {
            let data_len = f.read_u32::<BigEndian>()? as usize;
            let num_objects = f.read_u32::<BigEndian>()? as usize;
            f.read_u32::<BigEndian>()?;
            let whsize = f.read_u32::<BigEndian>()? as usize;

            (data_len, num_objects, whsize)
        }
        _ => {
            bail!("Invalid magic number: 0x{:08X}", magic);
        }
    };

    Ok(Header {
        data_len,
        num_objects,
        whsize,
    })
}

// The recursive solution below stack-overflows on large lists
// To fix this hackily, I've special cased lists
// If I need to do something more complicated I should probably come back and make
// this more similar to the C code (with unrolled recursion and an explicit stack, and using
// Rc wrappers for heap allocation
pub fn input_list<R: Read>(f: &mut R) -> Result<Vec<MLValue>> {
    let _ = parse_header(f)?;
    let mut result = vec![];

    loop {
        let code = f.read_u8()?;
        match code {
            // Small block, tag = 0, length = 2
            0xa0 => {
                result.push(read_value(f)?);
            }
            // Small int value = 0 (represents [])
            0x40 => {
                break;
            }
            _ => {
                bail!("Unexpected code for list: {}", code);
            }
        }
    }

    Ok(result)
}

fn read_value<R: Read>(f: &mut R) -> Result<MLValue> {
    let code = f.read_u8()?;

    let v = match code {
        PREFIX_SMALL_BLOCK..=PREFIX_SMALL_BLOCK_END => {
            let tag = code & 0xF;
            let size = ((code as usize) >> 4) & 0x7;

            read_block(f, tag, size)?
        }

        PREFIX_SMALL_INT..=PREFIX_SMALL_INT_END => MLValue::Int((code as i64) & 0x3f),

        PREFIX_SMALL_STRING..=PREFIX_SMALL_STRING_END => {
            let length = (code as usize) & 0x1F;
            read_string(f, length)?
        }

        CODE_INT8 => MLValue::Int(f.read_i8()? as i64),
        CODE_INT16 => MLValue::Int(f.read_i16::<BigEndian>()? as i64),
        CODE_INT32 => MLValue::Int(f.read_i32::<BigEndian>()? as i64),
        CODE_INT64 => MLValue::Int(f.read_i32::<BigEndian>()? as i64),

        CODE_SHARED8 => MLValue::Shared(f.read_u8()? as usize),
        CODE_SHARED16 => MLValue::Shared(f.read_u16::<BigEndian>()? as usize),
        CODE_SHARED32 => MLValue::Shared(f.read_u32::<BigEndian>()? as usize),
        CODE_SHARED64 => MLValue::Shared(f.read_u64::<BigEndian>()? as usize),

        CODE_BLOCK32 => {
            let header = f.read_u32::<BigEndian>()?;
            let size = header as usize >> 10;
            let tag = (header & 0xFF) as u8;

            return read_block(f, tag, size);
        }

        CODE_BLOCK64 => {
            let header = f.read_u64::<BigEndian>()?;
            let size = header as usize >> 10;
            let tag = (header & 0xFF) as u8;

            return read_block(f, tag, size);
        }

        CODE_STRING8 => {
            let size = f.read_u8()? as usize;

            return read_string(f, size);
        }

        CODE_STRING32 => {
            let size = f.read_u32::<BigEndian>()? as usize;

            return read_string(f, size);
        }

        CODE_STRING64 => {
            let size = f.read_u64::<BigEndian>()? as usize;

            return read_string(f, size);
        }

        CODE_DOUBLE_LITTLE | CODE_DOUBLE_BIG => MLValue::Double(f.read_f64::<BigEndian>()?),

        CODE_DOUBLE_ARRAY8_LITTLE | CODE_DOUBLE_ARRAY8_BIG => {
            bail!("Unimplemented: CODE_DOUBLE_ARRAY8_[LITTLE/BIG]");
        }

        CODE_DOUBLE_ARRAY32_LITTLE | CODE_DOUBLE_ARRAY32_BIG => {
            bail!("Unimplemented: CODE_DOUBLE_ARRAY32_[LITTLE/BIG]");
        }

        CODE_DOUBLE_ARRAY64_LITTLE | CODE_DOUBLE_ARRAY64_BIG => {
            bail!("Unimplemented: CODE_DOUBLE_ARRAY64_[LITTLE/BIG]");
        }

        CODE_CODEPOINTER => {
            bail!("Unimplemented: CODE_POINTER");
        }

        CODE_INFIXPOINTER => {
            bail!("Unimplemented: INFIX_POINTER");
        }

        CODE_CUSTOM | CODE_CUSTOM_FIXED | CODE_CUSTOM_LEN => {
            let identifier = read_c_string(f)?;

            match identifier.as_str() {
                "_i" => MLValue::Int32(f.read_i32::<BigEndian>()?),
                "_j" => MLValue::Int64(f.read_i64::<BigEndian>()?),
                "_n" => MLValue::Int64(f.read_i64::<BigEndian>()?),
                _ => {
                    bail!("Unimplemented: CODE_CUSTOM* for {}", identifier);
                }
            }
        }

        i => bail!("Unimplemented: code = {}", i),
    };

    Ok(v)
}

fn read_block<R: Read>(f: &mut R, tag: u8, size: usize) -> Result<MLValue> {
    let mut items = Vec::new();

    if tag == OBJECT_TAG {
        ensure!(size >= 2, "Object size less than 2");
        for _ in 0..(size - 2) {
            items.push(read_value(f)?);
        }

        items.insert(0, read_value(f)?);
        items.insert(1, read_value(f)?);
    } else {
        for _ in 0..size {
            items.push(read_value(f)?);
        }
    }

    Ok(MLValue::Block { tag, items })
}

fn read_string<R: Read>(f: &mut R, length: usize) -> Result<MLValue> {
    let mut buf = vec![0; length];
    f.read_exact(&mut buf)?;
    Ok(match String::from_utf8(buf.clone()) {
        Ok(s) => MLValue::StringUtf8(s),
        Err(_) => MLValue::StringBytes(buf),
    })
}

fn read_c_string<R: Read>(f: &mut R) -> Result<String> {
    let mut bytes = vec![];
    loop {
        let c = f.read_u8()?;
        if c == 0 {
            break;
        } else {
            bytes.push(c);
        }
    }

    Ok(String::from_utf8_lossy(&bytes).into())
}
