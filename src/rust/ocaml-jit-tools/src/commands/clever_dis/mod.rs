use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use structopt::clap::arg_enum;
use structopt::StructOpt;

use parsing::process_bytecode;

use crate::bytecode_files::parse_bytecode_file;
use crate::commands::clever_dis::ssa::translate_closure;

mod data;
mod parsing;
mod ssa;
mod visualisation;

arg_enum! {
    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    pub enum DotShow {
        Both,
        Bytecode,
        SSA,
    }
}

impl DotShow {
    pub fn show_bytecode(&self) -> bool {
        match self {
            DotShow::Both => true,
            DotShow::Bytecode => true,
            DotShow::SSA => false,
        }
    }

    pub fn show_ssa(&self) -> bool {
        match self {
            DotShow::Both => true,
            DotShow::Bytecode => false,
            DotShow::SSA => true,
        }
    }
}

#[derive(StructOpt)]
#[structopt(about = "disassemble bytecode files")]
pub struct Options {
    #[structopt(long, parse(from_os_str))]
    dot: Option<PathBuf>,

    #[structopt(long, short)]
    verbose: bool,

    #[structopt(long)]
    output_closure_json: bool,

    #[structopt(long)]
    no_relocate: bool,

    #[structopt(long)]
    print_debug: bool,

    #[structopt(long, possible_values = &DotShow::variants(), case_insensitive = true, default_value="Both")]
    dot_show: DotShow,

    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

// Main entrypoint

pub fn run(options: Options) -> Result<()> {
    let mut f = File::open(&options.input).with_context(|| {
        format!(
            "Problem opening bytecode file {}",
            &(options.input).to_string_lossy()
        )
    })?;
    let bcf = parse_bytecode_file(&mut f).context("Problem parsing bytecode file")?;

    let program = process_bytecode(bcf).context("Problem analysing parsed bytecode")?;

    let use_relocations = !options.no_relocate;

    let mut ssa_closures = vec![];
    for (closure_id, closure) in program.closures.iter().enumerate() {
        ssa_closures.push(
            translate_closure(closure, use_relocations)
                .with_context(|| format!("Problem translating closure {}", closure_id))?,
        );
    }

    if options.print_debug {
        println!("{:#?}", program);
    }

    if let Some(dot) = &options.dot {
        visualisation::write_dot_graphs(
            &program,
            &ssa_closures,
            visualisation::Options {
                use_links: true,
                verbose: options.verbose,
                show: options.dot_show,
                output_path: dot.clone(),
                output_closure_json: options.output_closure_json,
            },
        )
        .context("Problem writing visualisation graphs")?;
    }

    Ok(())
}
