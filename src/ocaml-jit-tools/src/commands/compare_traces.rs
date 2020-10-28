use crate::utils::die;
use colored::Colorize;
use ocaml_jit_shared::{compare_traces, TraceEntry};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "compare traces between the interpreter and the JIT")]
pub struct Options {
    #[structopt(parse(from_os_str))]
    bytecode_file: PathBuf,

    #[structopt(name = "ARGUMENTS")]
    other_args: Vec<OsString>,

    #[structopt(short = "q", long = "quiet")]
    quiet: bool,
}

type Result<O, E = Box<dyn Error>> = std::result::Result<O, E>;

pub fn run(options: Options) {
    let () = run_exn(options).unwrap_or_else(die);
}

fn run_exn(options: Options) -> Result<()> {
    let path = &options.bytecode_file;
    let mut compiled =
        RunningProgram::new(path, "-jt --trace-format JSON", options.other_args.iter())?;
    let mut interpreted =
        RunningProgram::new(path, "-t --trace-format JSON", options.other_args.iter())?;

    loop {
        let interpreted_output = interpreted.get_trace_line_or_exit(!options.quiet)?;
        let compiled_output = compiled.get_trace_line_or_exit(!options.quiet)?;

        if compiled_output != interpreted_output {
            match (&compiled_output, &interpreted_output) {
                (Output::Trace(compiled_trace), Output::Trace(interpreted_trace)) => {
                    println!("{}", "Difference in outputs!".red().bold());
                    compare_traces(interpreted_trace, compiled_trace);
                }
                _ => {
                    println!();
                    println!(
                        "{}",
                        format!("Interpreted: {}", interpreted_output.format()).bold()
                    );
                    println!("Compiled:    {}", compiled_output.format());
                    println!("{}", "One program exited early!".red().bold());
                }
            }
            std::process::exit(1);
        }

        match interpreted_output {
            Output::Trace(_) => {
                if !options.quiet {
                    println!("{}", interpreted_output.format().yellow().bold());
                    println!("{}", compiled_output.format());
                }
            }
            Output::Exited { exit_code } => {
                if !options.quiet {
                    println!("{}", format!("Exited: {}", exit_code).green().bold());
                }
                break;
            }
        }
    }

    Ok(())
}
#[derive(PartialEq, Debug)]
enum Output {
    Trace(TraceEntry),
    Exited { exit_code: i32 },
}

impl Output {
    fn format(&self) -> String {
        match self {
            Output::Trace(t) => t.format(),
            Output::Exited { exit_code } => format!("Exited with code {}", exit_code),
        }
    }
}

struct RunningProgram {
    child: process::Child,
    stdout: BufReader<process::ChildStdout>,
}

impl RunningProgram {
    fn new<S: AsRef<OsStr>, S2: AsRef<OsStr>, I: IntoIterator<Item = S2>>(
        s: S,
        jit_options: &'static str,
        other_args: I,
    ) -> Result<RunningProgram> {
        let mut child = process::Command::new(s)
            .args(other_args)
            .env("JIT_OPTIONS", jit_options)
            .stdout(process::Stdio::piped())
            .spawn()?;
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Ok(RunningProgram { child, stdout })
    }

    fn get_trace_line_or_exit(&mut self, show_output: bool) -> Result<Output> {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line)?;
            if read == 0 {
                let exit_code = self.child.wait()?.code().unwrap();
                return Ok(Output::Exited { exit_code });
            } else {
                if line.starts_with("!T!") {
                    let trace: TraceEntry =
                        serde_json::from_str(line.trim_start_matches("!T!")).unwrap();
                    if trace.location.is_bytecode() {
                        return Ok(Output::Trace(trace));
                    } else if show_output {
                        trace.print();
                    }
                } else if show_output {
                    print!("{}", line);
                }
            }
        }
    }
}
