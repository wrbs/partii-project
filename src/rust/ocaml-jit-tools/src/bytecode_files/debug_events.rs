use super::ml_data::input_value;
use super::trailer::{Trailer, DBUG_SECTION};
use crate::bytecode_files::ml_data::input_list;
use crate::bytecode_files::MLValue;
use anyhow::{bail, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;

pub struct DebugEventList {
    pub orig: u32,
    pub entries: Vec<MLValue>,
    pub absolute_dirs: MLValue,
}

pub struct DebugInfo {
    pub event_lists: Vec<DebugEventList>,
}

pub fn parse_debug_events(f: &mut File, trailer: &Trailer) -> Result<Option<DebugInfo>> {
    let mut section = match trailer.find_section(DBUG_SECTION) {
        Some(s) => s.read_section(f)?,
        None => return Ok(None),
    };

    let num_eventlists = section.read_u32::<BigEndian>()?;

    let mut event_lists = Vec::with_capacity(num_eventlists as usize);

    for _ in (0..num_eventlists) {
        let orig = section.read_u32::<BigEndian>()?;
        let entries = input_list(&mut section).context("Problem reading debug event value")?;
        let absolute_dirs =
            input_value(&mut section).context("Problem reading debug event value")?;

        event_lists.push(DebugEventList {
            orig,
            entries,
            absolute_dirs,
        });
    }

    Ok(Some(DebugInfo { event_lists }))
}
