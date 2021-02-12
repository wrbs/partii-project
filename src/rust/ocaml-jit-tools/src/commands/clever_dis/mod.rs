mod data;
mod parsing;
mod ssa;
mod visualisation;

use crate::bytecode_files::parse_bytecode_file;
use anyhow::{Context, Result};
use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

use parsing::process_bytecode;

#[derive(StructOpt)]
#[structopt(about = "disassemble bytecode files")]
pub struct Options {
    #[structopt(long, parse(from_os_str))]
    dot: Option<PathBuf>,

    #[structopt(long, short)]
    verbose: bool,

    #[structopt(long)]
    print_debug: bool,

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

    assumptions::validate_assumptions(&program)?;

    if options.print_debug {
        println!("{:#?}", program);
    }

    if let Some(dot) = &options.dot {
        visualisation::write_dot_graphs(
            &program,
            visualisation::Options {
                use_links: true,
                verbose: options.verbose,
                output_path: dot.clone(),
            },
        )
        .context("Problem writing visualisation graphs")?;
    }

    Ok(())
}

// Entrypoint to disassemble

mod assumptions {
    use crate::commands::clever_dis::data::Program;
    use anyhow::{bail, Result};
    use std::collections::HashMap;

    pub fn validate_assumptions(program: &Program) -> Result<()> {
        // Check that every closure has a unique path to it

        Ok(())
    }
}
