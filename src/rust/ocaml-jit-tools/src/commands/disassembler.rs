use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use structopt::clap::arg_enum;
use structopt::StructOpt;

use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction};

use crate::bytecode_files::{
    parse_bytecode_file, BytecodeFile, DebugInfo, MLValue, MLValueBlock, MLValueBlocks,
};

arg_enum! {
    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    enum ShowSections {
        All,
        Instructions,
        Primitives,
        GlobalData,
        SymbolTable,
        DebugEvents,
    }
}

impl ShowSections {
    fn should_show(&self, other: &ShowSections) -> bool {
        self == &ShowSections::All || self == other
    }
}

#[derive(StructOpt)]
#[structopt(about = "disassemble bytecode files")]
pub struct Options {
    #[structopt(long, possible_values = &ShowSections::variants(), case_insensitive = true, default_value="All")]
    show: ShowSections,

    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

pub fn run(options: Options) -> Result<()> {
    let mut f = File::open(options.input).context("Problem opening input file")?;

    let bcf = parse_bytecode_file(&mut f).context("Problem parsing bytecode file")?;

    if options.show.should_show(&ShowSections::Instructions) {
        show_instructions(&bcf)?;
    }

    if options.show.should_show(&ShowSections::Primitives) {
        show_primitives(&bcf.primitives);
        println!();
    }

    if options.show.should_show(&ShowSections::GlobalData) {
        show_global_data(&bcf.global_data_blocks, &bcf.global_data);
        println!();
    }

    if options.show.should_show(&ShowSections::SymbolTable) {
        show_symbol_table(&bcf.symbol_table);
        println!();
    }

    if options.show.should_show(&ShowSections::DebugEvents) {
        show_debug_events(&bcf.debug_info);
        println!();
    }

    Ok(())
}

fn show_instructions(bcf: &BytecodeFile) -> Result<()> {
    println!("{}", "Instructions:".red().bold());
    let mut instruction_count = None;

    for instruction in bcf.parse_instructions()?.iter() {
        if let Instruction::LabelDef(offset) = instruction {
            if instruction_count != None {
                println!();
            }
            instruction_count = Some(0);

            print!("{}\t", offset.0);
        } else if let Some(v) = instruction_count {
            if v >= 1 {
                print!(", ");
            }

            instruction_count = Some(v + 1);
            show_instruction(instruction);
        }
    }

    println!();

    Ok(())
}

fn show_instruction(instruction: &Instruction<BytecodeRelativeOffset>) {
    print!("{:?}", instruction.map_labels(|x| x.0));
}

fn show_primitives(primitives: &[String]) {
    println!("{}", "Primitives:".red().bold());
    let n = primitives.len();
    let width = (n as f32).log10() as usize;

    for (index, primitive) in primitives.iter().enumerate() {
        println!("{:width$} {}", index, primitive, width = width);
    }
    println!("{}", format!("count: {}", n).bright_black());
}

fn show_global_data(global_data_blocks: &MLValueBlocks, global_data: &MLValue) {
    println!("{}", "Global data:".red().bold());
    match global_data {
        MLValue::Block(block_id) => {
            let (tag, items) = global_data_blocks.get_block(block_id).unwrap();
            println!("Tag: {}", tag);
            let n = items.len();
            let width = (n as f32).log10() as usize;

            for (index, value) in items.iter().enumerate() {
                println!(
                    "{:width$} {}",
                    index,
                    global_data_blocks.format_value(value),
                    width = width
                );
            }

            println!("{}", format!("count: {}", n).bright_black());
        }
        _ => println!(
            "Not a block as expected - instead {}",
            global_data_blocks.format_value(global_data)
        ),
    }
}

fn show_symbol_table(symbol_table: &HashMap<usize, String>) {
    println!("{}", "Symbol table:".red().bold());
    let mut entries: Vec<(usize, &str)> =
        symbol_table.iter().map(|(n, s)| (*n, s.as_str())).collect();
    entries.sort();

    let n = entries.len();
    let width = (n as f32).log10() as usize;

    for (index, mapping) in entries {
        println!("{:width$} {}", index, mapping, width = width);
    }
    println!("{}", format!("count: {}", n).bright_black());
}

fn show_debug_events(debug_events: &Option<DebugInfo>) {
    println!("{}", "Symbol table:".red().bold());
    let events = match debug_events {
        Some(e) => e,
        None => {
            println!("{}", "No debug events included in bytecode".bright_black());
            return;
        }
    };

    for el in &events.event_lists {
        println!("Orig: {}", el.orig);
        println!("Entries:");
        for entry in &el.entries {
            println!("{:#?}", entry);
        }
        println!(
            "Absolute dirs: {}",
            el.absolute_dirs_blocks.format_value(&el.absolute_dirs)
        );
        println!();
    }

    println!(
        "{}",
        format!("count: {}", events.event_lists.len()).bright_black()
    );
}
