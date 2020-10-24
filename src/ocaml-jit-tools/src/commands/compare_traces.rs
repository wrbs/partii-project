use crate::utils::die;
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
}

type Result<O, E = Box<dyn Error>> = std::result::Result<O, E>;

pub fn run(options: Options) {
    let () = run_exn(options).unwrap_or_else(die);
}

fn run_exn(options: Options) -> Result<()> {
    let path = &options.bytecode_file;
    let mut compiled = RunningProgram::new(path, "-jt", options.other_args.iter())?;
    let mut interpreted = RunningProgram::new(path, "-t", options.other_args.iter())?;

    loop {
        let compiled_output = compiled.get_trace_line_or_exit(true)?;
        let interpreted_output = interpreted.get_trace_line_or_exit(false)?;

        if compiled_output != interpreted_output {
            println!("Mismatch between states!");
            println!("Compiled:    {:?}", compiled_output);
            println!("Interpreted: {:?}", interpreted_output);
            break;
        } else {
            match compiled_output {
                Output::Trace(s) => print!("{}", s),
                Output::Exited { exit_code } => {
                    println!("Exited: {}", exit_code);
                    break;
                }
            }
        }
    }

    Ok(())
}

#[derive(Eq, PartialEq, Debug)]
enum Output {
    Trace(String),
    Exited { exit_code: i32 },
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

    fn get_trace_line_or_exit(&mut self, print_non_matching: bool) -> Result<Output> {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line)?;
            if read == 0 {
                let exit_code = self.child.wait()?.code().unwrap();
                return Ok(Output::Exited { exit_code });
            } else {
                if line.starts_with("!T!") {
                    return Ok(Output::Trace(line));
                } else if print_non_matching {
                    print!("{}", line);
                }
            }
        }
    }
}
