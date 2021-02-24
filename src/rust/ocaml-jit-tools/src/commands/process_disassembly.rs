use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::iter::Peekable;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use structopt::StructOpt;

use ocaml_jit_shared::Instruction;

use crate::bytecode_files::parse_bytecode_file;

#[derive(StructOpt)]
#[structopt(about = "compare traces between the interpreter and the JIT")]
pub struct Options {
    #[structopt(long)]
    show_debug: bool,

    #[structopt(parse(from_os_str), long)]
    bytecode_file: PathBuf,

    #[structopt(parse(from_os_str), long)]
    dumpobj_output: PathBuf,
}

pub fn run(options: Options) -> Result<()> {
    let mut bcf_f = File::open(&options.bytecode_file)?;
    let bcf = parse_bytecode_file(&mut bcf_f)?;

    let dumpobj_f = BufReader::new(File::open(&options.dumpobj_output)?);
    // Ignore the first line
    let mut lines = dumpobj_f.lines().peekable();

    lines.next().ok_or(anyhow!("No first line"))??;

    let mut dumpobj_rest = String::new();
    let mut first = true;
    let mut extra_newline = false;
    for instruction in bcf.parse_instructions()?.iter() {
        match instruction {
            Instruction::LabelDef(offset) => {
                print!("\n{}", dumpobj_rest);
                if extra_newline {
                    println!();
                }
                if options.show_debug {
                    if let Some(e) = bcf
                        .debug_info
                        .as_ref()
                        .and_then(|di| di.events.get(&offset.0))
                    {
                        println!("{:#?}", e);
                    }
                }
                match get_line(&mut lines, options.show_debug)? {
                    Line::Instruction {
                        offset: dumpobj_offset,
                        dumpobj_output,
                        rest,
                    } => {
                        dumpobj_rest = rest;
                        print!("{:<10} {} -> ", dumpobj_offset, dumpobj_output);

                        if offset.0 != dumpobj_offset {
                            bail!("Invalid offsets: {} != {}", dumpobj_output, dumpobj_output);
                        }
                        first = true;
                        extra_newline = false;
                    }
                }
            }
            _ => {
                if first {
                    first = false;

                    match instruction {
                        Instruction::ApplyTerm(_, _)
                        | Instruction::Return(_)
                        | Instruction::Restart
                        | Instruction::Raise(_)
                        | Instruction::BranchCmp(_, _, _)
                        | Instruction::Branch(_)
                        | Instruction::BranchIf(_)
                        | Instruction::BranchIfNot(_)
                        | Instruction::Switch(_, _)
                        | Instruction::Stop => {
                            extra_newline = true;
                        }
                        _ => (),
                    }
                } else {
                    print!(", ");
                }
                print!("{:?}", instruction.map_labels(|x| x.0));
            }
        }
    }

    Ok(())
}

enum Line {
    Instruction {
        offset: usize,
        dumpobj_output: String,
        rest: String,
    },
}

fn get_line<R: BufRead>(lines: &mut Peekable<Lines<R>>, show_debug: bool) -> Result<Line> {
    let mut line = lines.next().ok_or(anyhow!("No first line"))??;

    while line.starts_with("File") {
        if show_debug {
            println!("{}", line);
        }
        line = lines.next().ok_or(anyhow!("No first line"))??;
    }

    let trimmed = line.trim();
    let mut sections = trimmed.splitn(2, ' ');
    let offset = sections.next().ok_or(anyhow!("No offset for line"))?;
    let offset: usize = offset.parse()?;
    let dumpobj_output = String::from(sections.next().ok_or(anyhow!("No rest for line"))?.trim());

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
                lines
                    .next()
                    .ok_or(anyhow!("Cannot get next line for problem"))??;
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

    Ok(Line::Instruction {
        offset,
        dumpobj_output,
        rest,
    })
}
