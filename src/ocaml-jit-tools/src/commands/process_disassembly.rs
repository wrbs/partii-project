use crate::bytecode_files::parse_bytecode_file;
use crate::utils::die;
use ocaml_jit_shared::Instruction;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::iter::Peekable;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "compare traces between the interpreter and the JIT")]
pub struct Options {
    #[structopt(parse(from_os_str), long)]
    bytecode_file: PathBuf,

    #[structopt(parse(from_os_str), long)]
    dumpobj_output: PathBuf,
}

type Result<O, E = Box<dyn Error>> = std::result::Result<O, E>;

pub fn run(options: Options) {
    let () = run_exn(options).unwrap_or_else(die);
}

fn run_exn(options: Options) -> Result<()> {
    let mut bcf_f = File::open(&options.bytecode_file)?;
    let bcf = parse_bytecode_file(&mut bcf_f)?;

    let dumpobj_f = BufReader::new(File::open(&options.dumpobj_output)?);
    // Ignore the first line
    let mut lines = dumpobj_f.lines().peekable();

    let _ = lines.next().ok_or("No first line")??;

    for (offset, parsed_instructions) in bcf.instructions {
        let (dumpobj_offset, dumpobj_output, dumpobj_rest) = get_line(&mut lines)?;
        if offset.0 != dumpobj_offset {
            return Err(
                format!("Invalid offsets: {} != {}", dumpobj_output, dumpobj_output).into(),
            );
        }

        print!("{:<10} {} -> ", dumpobj_offset, dumpobj_output);

        let mut first = true;
        for instruction in parsed_instructions.iter().map(|x| x.map_labels(|l| l.0)) {
            if first {
                first = false;
            } else {
                print!(", ");
            }
            print!("{:?}", instruction);
        }
        print!("\n{}", dumpobj_rest);
        match parsed_instructions[0] {
            Instruction::ApplyTerm(_, _)
            | Instruction::Return(_)
            | Instruction::Restart
            | Instruction::Raise(_)
            | Instruction::BranchCmp(_, _, _)
            | Instruction::Branch(_)
            | Instruction::BranchIf(_)
            | Instruction::BranchIfNot(_)
            | Instruction::Switch(_, _)
            | Instruction::Stop => println!(),
            _ => (),
        }
    }

    Ok(())
}

fn get_line<R: BufRead>(lines: &mut Peekable<Lines<R>>) -> Result<(usize, String, String)> {
    let line = lines.next().ok_or("No first line")??;

    let trimmed = line.trim();
    let mut sections = trimmed.splitn(2, ' ');
    let offset = sections.next().ok_or("No offset for line")?;
    let offset: usize = offset.parse()?;
    let dumpobj_output = String::from(sections.next().ok_or("No rest for line")?.trim());

    let mut rest = String::new();
    if dumpobj_output.starts_with("SWITCH") {
        loop {
            // Deal with errors
            let p = lines.peek();
            let problem = match p {
                Some(Err(_)) => true,
                _ => false,
            };
            if problem {
                // Parse the error we found
                lines.next().unwrap()?;
            }

            if let Some(l) = lines.peek() {
                let line = l.as_ref().unwrap().trim();
                if line.starts_with("int") || line.starts_with("tag") {
                    rest.push_str(&format!("           {}\n", line));
                } else {
                    break;
                }
            } else {
                break;
            }
            lines.next();
        }
    }

    Ok((offset, dumpobj_output, rest))
}
