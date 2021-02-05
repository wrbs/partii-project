use crate::bytecode_files::{ParseFileError, Trailer};
use std::fs::File;
use std::io::{BufRead, BufReader};

const PRIM_SECTION: &str = "PRIM";

pub fn parse_primitives(f: &mut File, trailer: &Trailer) -> Result<Vec<String>, ParseFileError> {
    let section = match trailer.find_section(PRIM_SECTION) {
        None => return Ok(vec![]),
        Some(s) => s,
    };

    let mut section_read = BufReader::new(section.read_section(f)?);

    let mut result = Vec::new();

    loop {
        let mut read_buf = Vec::new();
        // read up to the null byte or eof - includes it if it finds the null byte
        section_read.read_until(0, &mut read_buf)?;

        if read_buf.is_empty() {
            break;
        } else if read_buf[read_buf.len() - 1] == 0 {
            // Parse as UTF8 string and add it
            result.push(String::from_utf8_lossy(&read_buf[..read_buf.len() - 1]).into());
        } else {
            return Err(ParseFileError::BadPrimitiveFormatting);
        }
    }

    Ok(result)
}
