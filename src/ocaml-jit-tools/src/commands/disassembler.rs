use crate::utils::die;
use colored::Colorize;

use crate::bytecode_files::{parse_bytecode_file, BytecodeFile};
use ocaml_jit_shared::Instruction;
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

    show_instructions(&bcf);
    show_primitives(&bcf.primitives);
}

fn show_instructions(bcf: &BytecodeFile) {
    println!("{}", "Instructions:".red().bold());
    for (offset, instructions) in bcf.instructions.iter() {
        print!("{}\t", offset);

        for (count, instruction) in instructions.iter().enumerate() {
            if count > 0 {
                print!(", ")
            }
            show_instruction(instruction);
        }

        println!();
    }
}

fn show_instruction(instruction: &Instruction<usize>) {
    print!("{:?}", instruction);
}

fn show_primitives(primitives: &[String]) {
    println!("{}", "Primitives:".red().bold());
    for primitive in primitives {
        println!("{}", primitive)
    }
    println!("{}", format!("count: {}", primitives.len()).bright_black());
}
