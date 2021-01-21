use anyhow::{Context, Result};
use colored::Colorize;
use ocaml_jit_shared::{compare_traces, TraceEntry};
use os_pipe::{pipe, PipeReader};
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "compare traces between the interpreter and the JIT")]
#[structopt(setting = structopt::clap::AppSettings::TrailingVarArg)]
pub struct Options {
    #[structopt(parse(from_os_str))]
    bytecode_file: PathBuf,

    #[structopt(short = "q", long = "quiet")]
    quiet: bool,

    #[structopt(short = "p", long = "ocamlrunparams")]
    ocaml_run_params: Option<OsString>,

    #[structopt(name = "ARGUMENTS")]
    other_args: Vec<OsString>,
}

pub fn run(options: Options) -> Result<()> {
    let mut retry_attempts = 3;

    while retry_attempts > 0 {
        match execute(&options)? {
            TestResult::Pass => {
                break;
            }
            TestResult::Fail => {
                process::exit(1);
            }
            TestResult::FailFirstLine => {
                println!("{}", format!("Failed after first line, retrying to see if we get a luckier initial memory allocation - {} remaining", retry_attempts).bright_blue().bold());
                retry_attempts -= 1;
            }
        }
    }

    Ok(())
}

enum TestResult {
    Pass,
    Fail,
    FailFirstLine,
}

fn execute(options: &Options) -> Result<TestResult> {
    let path = &options.bytecode_file;
    let ocaml_run_params = options
        .ocaml_run_params
        .clone()
        .unwrap_or_else(OsString::new);
    let mut compiled = RunningProgram::new(
        path,
        "-jt --trace-format JSON",
        options.other_args.iter(),
        &ocaml_run_params,
        false,
        !options.quiet,
    )
    .context("Problem starting jit program")?;
    let mut interpreted = RunningProgram::new(
        path,
        "-t --trace-format JSON",
        options.other_args.iter(),
        &ocaml_run_params,
        true,
        !options.quiet,
    )
    .context("Problem starting non-jit program")?;

    let mut first_line_passed = false;

    loop {
        let interpreted_output = interpreted.get_trace_line_or_exit()?;
        let compiled_output = compiled.get_trace_line_or_exit()?;

        if !options.quiet {
            println!();
        }

        if compiled_output != interpreted_output {
            match (&compiled_output, &interpreted_output) {
                (Output::Trace(compiled_trace), Output::Trace(interpreted_trace)) => {
                    println!("{}", "Difference in outputs!".red().bold());
                    compare_traces(interpreted_trace, compiled_trace);
                }
                _ => {
                    println!("{}", interpreted_output.format().yellow().bold());
                    println!("{}", compiled_output.format());
                    println!("{}", "One program exited early!".red().bold());
                }
            }

            if first_line_passed {
                return Ok(TestResult::Fail);
            } else {
                return Ok(TestResult::FailFirstLine);
            }
        }

        first_line_passed = true;

        match interpreted_output {
            Output::Trace(_) => {
                if !options.quiet {
                    println!("{}", interpreted_output.format().yellow().bold());
                    println!("{}", compiled_output.format());
                }
            }
            Output::Exited => {
                if !options.quiet {
                    println!("{}", "Exited".green().bold());
                }
                break;
            }
        }
    }

    Ok(TestResult::Pass)
}

#[derive(PartialEq, Debug)]
enum Output {
    Trace(TraceEntry),
    Exited,
}

impl Output {
    fn format(&self) -> String {
        match self {
            Output::Trace(t) => t.format(),
            Output::Exited => String::from("Exited"),
        }
    }
}

struct RunningProgram {
    output: BufReader<PipeReader>,
    is_gold_standard: bool,
    show_output: bool,
}

impl RunningProgram {
    fn new<S: AsRef<OsStr>, S2: AsRef<OsStr>, S3: AsRef<OsStr>, I: IntoIterator<Item = S2>>(
        s: S,
        jit_options: &'static str,
        other_args: I,
        ocaml_run_params: S3,
        is_gold_standard: bool,
        show_output: bool,
    ) -> Result<RunningProgram> {
        let (reader, writer) = pipe()?;
        let writer_clone = writer.try_clone()?;

        let child = process::Command::new(s)
            .args(other_args)
            .env("JIT_OPTIONS", jit_options)
            .env("OCAMLRUNPARAM", ocaml_run_params)
            .stdout(writer)
            .stderr(writer_clone)
            .spawn()?;

        std::mem::drop(child);

        let output = BufReader::new(reader);
        Ok(RunningProgram {
            output,
            show_output,
            is_gold_standard,
        })
    }

    fn get_trace_line_or_exit(&mut self) -> Result<Output> {
        let mut line = String::new();
        loop {
            let read = self.output.read_line(&mut line)?;
            if read == 0 {
                return Ok(Output::Exited);
            } else if line.starts_with("!T!") {
                let trace: TraceEntry =
                    serde_json::from_str(line.trim_start_matches("!T!")).unwrap();
                if trace.location.is_bytecode() {
                    return Ok(Output::Trace(trace));
                } else if self.show_output {
                    trace.print();
                }
            } else if self.show_output {
                if self.is_gold_standard {
                    print!("{}", line.yellow().bold());
                } else {
                    print!("{}", line);
                }
            }
            line.clear();
        }
    }
}
