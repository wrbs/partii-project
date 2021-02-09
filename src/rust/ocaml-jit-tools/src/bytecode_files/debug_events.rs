use super::ml_data::input_value;
use super::trailer::{Trailer, DBUG_SECTION};
use crate::bytecode_files::{MLValue, MLValueBlocks};
use anyhow::{bail, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;

pub struct DebugEventList {
    pub orig: u32,
    pub entries_blocks: MLValueBlocks,
    pub entries: Vec<MLValue>,
    pub absolute_dirs_blocks: MLValueBlocks,
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
        let (list_blocks, list_value) =
            input_value(&mut section).context("Problem reading debug events")?;
        let entries = parse_event_list(&list_blocks, &list_value)
            .context("Problem parsing events from the MLValue")?;
        let (absolute_dirs_blocks, absolute_dirs) =
            input_value(&mut section).context("Problem reading debug event value")?;

        event_lists.push(DebugEventList {
            orig,
            entries_blocks: list_blocks,
            entries,
            absolute_dirs_blocks,
            absolute_dirs,
        });
    }

    Ok(Some(DebugInfo { event_lists }))
}

fn parse_event_list(_blocks: &MLValueBlocks, list: &MLValue) -> Result<Vec<MLValue>> {
    let mut events = vec![];
    let mut current_value = list;

    loop {
        match &current_value {
            MLValue::Int(0) => {
                return Ok(events);
            }
            MLValue::Block { tag: 0, items } => match &items[..] {
                [a, b] => {
                    events.push(a.clone());
                    current_value = b;
                }
                _ => bail!("Unexpected block in event in list"),
            },

            _ => bail!("Unexpected value in event list"),
        }
    }
}
