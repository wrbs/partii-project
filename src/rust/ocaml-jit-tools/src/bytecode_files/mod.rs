use std::collections::HashMap;
use std::fs::File;

use anyhow::{Context, Result};

pub use debug_events::DebugInfo;
pub use error::ParseFileError;
pub use ml_data::{MLValue, MLValueBlock, MLValueBlocks, MLValueString};
use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction};
pub use trailer::Trailer;
use trailer::{CODE_SECTION, DATA_SECTION};

mod bytecode;
pub mod debug_events;
mod error;
pub mod ml_data;
mod primitives;
mod symbol_table;
pub mod trailer;

pub struct BytecodeFile {
    pub trailer: Trailer,
    pub primitives: Vec<String>,
    pub code: Vec<u8>,
    pub global_data_blocks: MLValueBlocks,
    pub global_data: MLValue,
    pub symbol_table: HashMap<usize, String>,
    pub debug_info: Option<DebugInfo>,
}

pub fn parse_bytecode_file(f: &mut File) -> Result<BytecodeFile> {
    let trailer = trailer::parse_trailer(f).context("Problem parsing trailer")?;
    let primitives =
        primitives::parse_primitives(f, &trailer).context("Problem parsing primitives")?;

    let code = trailer
        .find_required_section(CODE_SECTION)?
        .read_section_vec(f)
        .context("Problem reading code section")?;

    let (global_data_blocks, global_data) = {
        let mut data_section = trailer
            .find_required_section(DATA_SECTION)?
            .read_section(f)?;
        ml_data::input_value(&mut data_section).context("Problem parsing global data")?
    };

    let symbol_table =
        symbol_table::parse_symbol_table(f, &trailer).context("Problem parsing symbol table")?;

    let debug_events =
        debug_events::parse_debug_events(f, &trailer).context("Problem parsing debug events")?;

    Ok(BytecodeFile {
        trailer,
        primitives,
        code,
        global_data_blocks,
        global_data,
        symbol_table,
        debug_info: debug_events,
    })
}

impl BytecodeFile {
    pub fn parse_instructions(
        &self,
    ) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, ParseFileError> {
        bytecode::parse_bytecode(&self.code, &self.primitives)
    }
}
