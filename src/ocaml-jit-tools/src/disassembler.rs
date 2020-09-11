use crate::utils::die;
use colored::Colorize;
use ocaml_bytecode::parse_bytecode_file;

use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "disassemble bytecode files")]
pub struct Options {
    #[structopt(parse(from_os_str))]
    input: PathBuf,
}

pub fn run(options: Options) {
    let mut f = File::open(options.input).unwrap_or_else(die);

    let bcf = parse_bytecode_file(&mut f).unwrap_or_else(die);

    show_primitives(&bcf.primitives);
}

fn show_primitives(primitives: &[String]) {
    println!("{}", "Primitives:".red().bold());
    for primitive in primitives {
        println!("{}", primitive)
    }
    println!("{}", format!("count: {}", primitives.len()).bright_black());
}
