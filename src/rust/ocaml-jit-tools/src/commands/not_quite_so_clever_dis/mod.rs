use std::{fs::File, path::PathBuf};

use anyhow::{Context, Result};
use structopt::StructOpt;

use crate::bytecode_files::parse_bytecode_file;
use ocaml_jit_shared::basic_blocks::parse_to_basic_blocks;
use std::collections::HashMap;

mod visualisation;

#[derive(StructOpt)]
#[structopt(about = "disassemble bytecode files")]
pub struct Options {
    #[structopt(long, parse(from_os_str))]
    dot: Option<PathBuf>,

    #[structopt(long)]
    output_closure_json: bool,

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

    let mut closures = HashMap::new();
    let mut closures_todo = vec![0];
    while let Some(entrypoint) = closures_todo.pop() {
        let closure = parse_to_basic_blocks(&bcf.code, entrypoint)
            .with_context(|| format!("Parsing closure with entrypoint {}", entrypoint))?;
        let todo = closure.used_closures.clone();
        closures.insert(entrypoint, closure);
        for referenced_closure in todo {
            if !closures.contains_key(&referenced_closure) {
                closures_todo.push(referenced_closure);
            }
        }
    }

    if options.print_debug {
        println!("{:#?}", closures);
    }

    if let Some(dot) = &options.dot {
        visualisation::write_dot_graphs(
            &closures,
            visualisation::Options {
                use_links: true,
                verbose: options.verbose,
                output_path: dot.clone(),
                output_closure_json: options.output_closure_json,
            },
        )
        .context("Problem writing visualisation graphs")?;
    }

    Ok(())
}
