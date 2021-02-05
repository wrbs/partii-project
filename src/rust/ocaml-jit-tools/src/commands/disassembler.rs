use crate::utils::die;
use colored::Colorize;

use crate::bytecode_files::{parse_bytecode_file, BytecodeFile};
use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction};
use std::fs::File;
use std::path::PathBuf;
use structopt::clap::arg_enum;
use structopt::StructOpt;

arg_enum! {
    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    enum ShowSections {
        All,
        Instructions,
        Primitives
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

pub fn run(options: Options) {
    let mut f = File::open(options.input).unwrap_or_else(die);

    let bcf = parse_bytecode_file(&mut f).unwrap_or_else(die);

    if options.show.should_show(&ShowSections::Instructions) {
        show_instructions(&bcf);
    }

    if options.show.should_show(&ShowSections::Primitives) {
        show_primitives(&bcf.primitives);
    }
}

fn show_instructions(bcf: &BytecodeFile) {
    println!("{}", "Instructions:".red().bold());
    let mut instruction_count = None;
    for instruction in bcf.instructions.iter() {
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
}

fn show_instruction(instruction: &Instruction<BytecodeRelativeOffset>) {
    print!("{:?}", instruction.map_labels(|x| x.0));
}

fn show_primitives(primitives: &[String]) {
    println!("{}", "Primitives:".red().bold());
    for primitive in primitives {
        println!("{}", primitive)
    }
    println!("{}", format!("count: {}", primitives.len()).bright_black());
}
