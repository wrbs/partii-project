use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::rc::Rc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};

use crate::bytecode_files::{MLValue, MLValueBlocks, MLValueString};

use super::ml_data::input_value;
use super::trailer::{Trailer, DBUG_SECTION};

pub struct DebugEventList {
    pub orig: u32,
    pub entries: Vec<DebugEvent>,
    pub absolute_dirs_blocks: MLValueBlocks,
    pub absolute_dirs: MLValue,
}

#[derive(Debug, Clone)]
pub struct DebugEvent {
    pub position: usize,
    pub module: Rc<String>,
    pub span: DebugSpan,
    pub kind: DebugEventKind,
    pub def_name: Rc<String>,
    pub info: DebugEventInfo,
    // pub type_env: usize,
    // pub type_subst: usize,
    // pub comp_env: usize,
    pub heap_env: Vec<(i64, Ident)>,
    pub rec_env: Vec<(i64, Ident)>,
    pub stack_size: usize,
    pub repr: DebugEventRepr,
}

#[derive(Debug, Clone)]
pub enum Ident {
    Local {
        name: Rc<String>,
        stamp: i64,
    },
    Scoped {
        name: Rc<String>,
        stamp: i64,
        scope: i64,
    },
    Global {
        name: Rc<String>,
    },
    Predef {
        name: Rc<String>,
        stamp: i64,
    },
}

impl Ident {
    fn name(&self) -> &Rc<String> {
        match self {
            Ident::Local { name, .. } => name,
            Ident::Scoped { name, .. } => name,
            Ident::Global { name, .. } => name,
            Ident::Predef { name, .. } => name,
        }
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone)]
pub struct DebugPosition {
    pub filename: Rc<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct DebugSpan {
    pub start: DebugPosition,
    pub end: DebugPosition,
}

#[derive(Debug, Copy, Clone)]
pub enum DebugEventKind {
    Before,
    After(usize),
    Pseudo,
}

#[derive(Debug, Copy, Clone)]
pub enum DebugEventInfo {
    Function,
    Return(usize),
    Other,
}

#[derive(Debug, Copy, Clone)]
pub enum DebugEventRepr {
    None,
    Parent(usize),
    Child(usize),
}

pub struct DebugInfo {
    pub event_lists: Vec<DebugEventList>,
    pub events: HashMap<usize, DebugEvent>,
}

pub fn parse_debug_events(f: &mut File, trailer: &Trailer) -> Result<Option<DebugInfo>> {
    let mut section = match trailer.find_section(DBUG_SECTION) {
        Some(s) => s.read_section(f)?,
        None => return Ok(None),
    };

    let num_eventlists = section.read_u32::<BigEndian>()?;

    let mut event_lists = Vec::with_capacity(num_eventlists as usize);

    for _ in 0..num_eventlists {
        let orig = section.read_u32::<BigEndian>()?;
        let (list_blocks, list_value) =
            input_value(&mut section).context("Problem reading debug events")?;
        let entries = parse_event_list(&list_blocks, &list_value, orig as usize)
            .context("Problem parsing events from the MLValue")?;
        let (absolute_dirs_blocks, absolute_dirs) =
            input_value(&mut section).context("Problem reading debug event value")?;

        event_lists.push(DebugEventList {
            orig,
            entries,
            absolute_dirs_blocks,
            absolute_dirs,
        });
    }

    let mut events = HashMap::new();

    for event_list in &event_lists {
        for event in &event_list.entries {
            events.insert(event.position, event.clone());
            // let prev = events.insert(event.position, event.clone());
            // if let Some(other) = prev {
            //     eprintln!(
            //         "Duplicate events at same position:\n{:#?}\n{:#?}",
            //         other, event
            //     );
            // }
        }
    }

    Ok(Some(DebugInfo {
        event_lists,
        events,
    }))
}

struct Strings<'a> {
    blocks: &'a MLValueBlocks,
    strings: HashMap<usize, Rc<String>>,
}

impl<'a> Strings<'a> {
    fn new(blocks: &'a MLValueBlocks) -> Strings<'a> {
        Strings {
            blocks,
            strings: HashMap::new(),
        }
    }

    fn get<U: Borrow<usize>>(&mut self, id: U) -> Result<Rc<String>> {
        let id = *id.borrow();
        match self.strings.get(&id) {
            Some(s) => Ok(s.clone()),
            None => {
                let old_s = match self.blocks.strings.get(id) {
                    Some(MLValueString::UTF8(s)) => s,
                    _ => bail!("Could not find/invalid UTF8 for string with id {}", id),
                };
                let s1 = Rc::new(old_s.clone());
                let s2 = s1.clone();
                self.strings.insert(id, s1);
                Ok(s2)
            }
        }
    }

    fn get_from_value(&mut self, value: &MLValue) -> Result<Rc<String>> {
        match value {
            MLValue::String(id) => self.get(id),
            _ => bail!("Expected string but found other type of value"),
        }
    }
}

fn parse_event_list(
    blocks: &MLValueBlocks,
    list: &MLValue,
    relocation_offset: usize,
) -> Result<Vec<DebugEvent>> {
    let mut events = vec![];
    let mut strings = Strings::new(blocks);
    let mut current_value = list;

    loop {
        match &current_value {
            MLValue::Int(0) => {
                return Ok(events);
            }
            MLValue::Block(block_id) => match blocks.get_block(block_id) {
                Some((0, [a, b])) => {
                    events.push(parse_debug_event(
                        blocks,
                        &mut strings,
                        a,
                        relocation_offset,
                    )?);
                    current_value = b;
                }
                _ => bail!("Unexpected value in event list"),
            },

            _ => bail!("Unexpected value in event list"),
        }
    }
}

fn parse_debug_event(
    blocks: &MLValueBlocks,
    strings: &mut Strings,
    event: &MLValue,
    relocation_offset: usize,
) -> Result<DebugEvent> {
    let block_id = match event {
        MLValue::Block(block_id) => *block_id,
        _ => bail!("Invalid debug event"),
    };

    let (tag, items) = blocks
        .get_block(block_id)
        .ok_or_else(|| anyhow!("Bad block id"))?;

    ensure!(tag == 0);
    ensure!(items.len() == 11);

    let ev_pos = match items[0] {
        MLValue::Int(i) => i as usize,
        _ => bail!("Invalid position value"),
    };

    let position = (ev_pos + relocation_offset) / 4;

    let module = strings.get_from_value(&items[1])?;

    let span = parse_span(blocks, strings, &items[2])?;

    let kind = match &items[3] {
        MLValue::Int(0) => DebugEventKind::Before,
        MLValue::Int(1) => DebugEventKind::Pseudo,
        MLValue::Block(block_id) => DebugEventKind::After(*block_id),
        o => bail!("Invalid kind value, {}", blocks.format_value(o)),
    };

    let def_name = strings.get_from_value(&items[4])?;

    let info = match &items[5] {
        MLValue::Int(0) => DebugEventInfo::Function,
        MLValue::Int(1) => DebugEventInfo::Other,
        MLValue::Block(block_id) => match blocks.get_block(block_id) {
            Some((0, &[MLValue::Int(i)])) => DebugEventInfo::Return(i as usize),
            _ => bail!("Invalid return branch value for debug info"),
        },
        o => bail!("Invalid info value, {}", blocks.format_value(o)),
    };

    let (heap_env, rec_env) = match &items[8] {
        MLValue::Block(b) => match blocks.get_block(b) {
            Some((0, [_ce_stack, ce_heap, ce_rec])) => (
                parse_env_table(blocks, strings, ce_heap)?,
                parse_env_table(blocks, strings, ce_rec)?,
            ),
            _ => bail!("Invalid compenv"),
        },
        _ => bail!("Invalid compenv"),
    };

    let stack_size = match &items[9] {
        MLValue::Int(i) => *i as usize,
        _ => bail!("Invalid stack size"),
    };

    let repr = match &items[10] {
        MLValue::Int(0) => DebugEventRepr::None,
        MLValue::Block(block_id) => match blocks.get_block(block_id) {
            Some((0, &[MLValue::Block(other_block)])) => DebugEventRepr::Parent(other_block),
            Some((1, &[MLValue::Block(other_block)])) => DebugEventRepr::Child(other_block),
            _ => bail!("invalid repr value"),
        },
        o => bail!("Invalid repr value, {}", blocks.format_value(o)),
    };

    Ok(DebugEvent {
        position,
        span,
        module,
        kind,
        def_name,
        info,
        heap_env,
        rec_env,
        stack_size,
        repr,
    })
}

fn parse_span(blocks: &MLValueBlocks, strings: &mut Strings, value: &MLValue) -> Result<DebugSpan> {
    match value {
        MLValue::Block(b) => match blocks.get_block(b) {
            Some((0, [s, e, _])) => Ok(DebugSpan {
                start: parse_position(blocks, strings, s)?,
                end: parse_position(blocks, strings, e)?,
            }),
            _ => bail!("Invalid span"),
        },
        _ => bail!("Invalid span"),
    }
}

fn parse_position(
    blocks: &MLValueBlocks,
    strings: &mut Strings,
    value: &MLValue,
) -> Result<DebugPosition> {
    match value {
        MLValue::Block(b) => match blocks.get_block(b) {
            Some((
                0,
                [MLValue::String(fname), MLValue::Int(lnum), MLValue::Int(bol), MLValue::Int(cnum)],
            )) => Ok(DebugPosition {
                filename: strings.get(fname)?,
                line: *lnum as usize,
                column: (*cnum - *bol) as usize,
            }),
            _ => bail!("Invalid position"),
        },
        _ => bail!("Invalid position"),
    }
}

fn parse_env_table(
    blocks: &MLValueBlocks,
    strings: &mut Strings,
    value: &MLValue,
) -> Result<Vec<(i64, Ident)>> {
    let mut stack = Vec::new();
    let mut found = Vec::new();
    stack.push(value);

    while let Some(b) = stack.pop() {
        match b {
            MLValue::Int(0) => continue,
            MLValue::Block(b) => match blocks.get_block(b) {
                Some((0, [l, MLValue::Block(data_block), r, MLValue::Int(_)])) => {
                    stack.push(l);
                    stack.push(r);

                    match blocks.get_block(data_block) {
                        Some((0, [ident_value, MLValue::Int(data), _])) => {
                            let ident = parse_ident(blocks, strings, ident_value)?;
                            found.push((*data, ident));
                        }
                        _ => bail!("Invalid env table"),
                    }
                }
                _ => bail!("Invalid env table"),
            },
            _ => bail!("Invalid env table"),
        }
    }

    found.sort_by_key(|(i, _)| *i);

    Ok(found)
}

fn parse_ident(blocks: &MLValueBlocks, strings: &mut Strings, value: &MLValue) -> Result<Ident> {
    match value {
        MLValue::Block(b) => match blocks.get_block(b) {
            Some((0, [MLValue::String(name_s), MLValue::Int(stamp)])) => Ok(Ident::Local {
                name: strings.get(name_s)?,
                stamp: *stamp,
            }),
            Some((1, [MLValue::String(name_s), MLValue::Int(stamp), MLValue::Int(scope)])) => {
                Ok(Ident::Scoped {
                    name: strings.get(name_s)?,
                    stamp: *stamp,
                    scope: *scope,
                })
            }
            Some((2, [MLValue::String(name_s)])) => Ok(Ident::Global {
                name: strings.get(name_s)?,
            }),
            Some((3, [MLValue::String(name_s), MLValue::Int(stamp)])) => Ok(Ident::Predef {
                name: strings.get(name_s)?,
                stamp: *stamp,
            }),
            _ => bail!("Invalid ident"),
        },
        _ => bail!("Invalid ident"),
    }
}
