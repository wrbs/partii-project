use std::{
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader},
    path::PathBuf,
    process,
};

use anyhow::{Context, Result};
use colored::Colorize;
use os_pipe::{pipe, PipeReader};
use structopt::StructOpt;

use ocaml_jit_shared::{
    call_trace::{compare_call_traces, CallTrace},
    compare_instruction_traces, InstructionTraceEntry, InstructionTraceLocation, Opcode,
};

#[derive(StructOpt)]
#[structopt(about = "compare call traces between the slower and the optimised JIT")]
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

    let mut dynasm = RunningProgram::new(
        path,
        "-j --call-trace --no-hot-threshold --trace-format JSON",
        options.other_args.iter(),
        &ocaml_run_params,
        true,
        !options.quiet,
    )
    .context("Problem starting program")?;
    let mut cranelift = RunningProgram::new(
        path,
        "-j --call-trace --hot-threshold 0 --trace-format JSON",
        options.other_args.iter(),
        &ocaml_run_params,
        false,
        !options.quiet,
    )
    .context("Problem starting program")?;

    let mut first_line_passed = false;

    loop {
        let dynasm_output = dynasm.get_trace_line_or_exit()?;
        let cranelift_output = cranelift.get_trace_line_or_exit()?;

        if !options.quiet {
            println!();
        }

        if dynasm_output != cranelift_output {
            match (&cranelift_output, &dynasm_output) {
                (Output::Trace(cranelift_trace), Output::Trace(dynasm_trace)) => {
                    println!("{}", "Difference in outputs!".red().bold());
                    compare_call_traces(dynasm_trace, cranelift_trace);
                }
                _ => {
                    println!("{}", dynasm_output.format().yellow().bold());
                    println!("{}", cranelift_output.format());
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

        match cranelift_output {
            Output::Trace(_) => {
                if !options.quiet {
                    println!("{}", cranelift_output.format().yellow().bold());
                    println!("{}", dynasm_output.format());
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
    Trace(CallTrace),
    Exited,
}

impl Output {
    fn format(&self) -> String {
        match self {
            Output::Trace(t) => format!("{}", t),
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
            } else if line.starts_with("!C!") {
                let trace: CallTrace =
                    serde_json::from_str(line.trim_start_matches("!C!")).unwrap();

                return Ok(Output::Trace(trace));
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
