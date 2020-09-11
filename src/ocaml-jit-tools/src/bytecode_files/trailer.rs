use std::fs::File;
use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use io::{Read, Seek};

use crate::bytecode_files::ParseFileError;

pub const EXEC_MAGIC: &[u8] = b"Caml1999X028";
pub const EXEC_MAGIC_LENGTH: usize = EXEC_MAGIC.len();
pub const TRAILER_LENGTH: usize = 4 + EXEC_MAGIC_LENGTH;

#[derive(Debug, Copy, Clone, Default)]
pub struct SectionEntry {
    pub name: [u8; 4],
    pub offset_from_end: usize,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct Trailer {
    pub sections: Vec<SectionEntry>,
}

pub fn parse_trailer(f: &mut File) -> Result<Trailer, ParseFileError> {
    f.seek(io::SeekFrom::End(-(TRAILER_LENGTH as i64)))?;
    let num_sections = f.read_u32::<BigEndian>()? as usize;

    // Check magic bytes at end
    let mut read_magic = [0u8; EXEC_MAGIC_LENGTH];
    if f.read(&mut read_magic)? != EXEC_MAGIC_LENGTH {
        return Err(ParseFileError::BadSize("could not read magic"));
    }
    if read_magic != EXEC_MAGIC {
        return Err(ParseFileError::WrongMagic);
    }

    // Read in the section header
    let toc_size = num_sections as usize * 8;
    f.seek(io::SeekFrom::End(-((TRAILER_LENGTH + toc_size) as i64)))?;

    let mut sections = Vec::with_capacity(num_sections);

    // Load up the section table with sizes
    for _ in 0..num_sections {
        let mut entry = SectionEntry::default();

        if f.read(&mut entry.name)? != 4 {
            return Err(ParseFileError::BadSize(
                "Could not read full section entry name",
            ));
        }

        entry.length = f.read_u32::<BigEndian>()? as usize;
        sections.push(entry);
    }

    // Work out file offsets
    let mut current_offset = TRAILER_LENGTH + toc_size;
    for i in (0..num_sections).rev() {
        let entry = &mut sections[i];
        current_offset += entry.length;
        entry.offset_from_end = current_offset;

        if entry.length > entry.offset_from_end {
            return Err(ParseFileError::BadSize(
                "Length is greater than offset from end",
            ));
        }
    }

    Ok(Trailer { sections })
}

impl Trailer {
    pub fn find_section(&self, name: &str) -> Option<&SectionEntry> {
        let name_bytes = name.as_bytes();

        self.sections.iter().find(|s| s.name == name_bytes)
    }

    pub fn find_required_section(
        &self,
        name: &'static str,
    ) -> Result<&SectionEntry, ParseFileError> {
        match self.find_section(name) {
            Some(s) => Ok(s),
            None => Err(ParseFileError::SectionNotFound(name)),
        }
    }
}

impl SectionEntry {
    pub fn seek_to(&self, f: &mut File) -> Result<(), ParseFileError> {
        f.seek(io::SeekFrom::End(-(self.offset_from_end as i64)))?;

        Ok(())
    }

    pub fn read_section<'a>(
        &self,
        f: &'a mut File,
    ) -> Result<ReadAtMost<&'a mut File>, ParseFileError> {
        self.seek_to(f)?;

        Ok(ReadAtMost::new(f, self.length))
    }
}

pub struct ReadAtMost<R: Read> {
    source: R,
    to_read: usize,
}

impl<R: Read> ReadAtMost<R> {
    pub fn new(source: R, maximum_bytes: usize) -> ReadAtMost<R> {
        ReadAtMost {
            source,
            to_read: maximum_bytes,
        }
    }

    pub fn release(self) -> R {
        self.source
    }
}

impl<R: Read> Read for ReadAtMost<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let to_read = std::cmp::min(buf.len(), self.to_read);
        let bytes_read = self.source.read(&mut buf[..to_read])?;

        self.to_read -= bytes_read;

        Ok(bytes_read)
    }
}
