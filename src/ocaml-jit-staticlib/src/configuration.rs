/*
 *
 * Options are set by setting the JIT_OPTIONS environment variable to an argument string which
 * is parsed to avoid impacting the runtime and OCaml program\*/

use clap::arg_enum;
use std::env;
use structopt::StructOpt;

const PROGRAM_NAME: &str = "ocaml-jit";
const ENV_VAR_KEY: &str = "JIT_OPTIONS";

arg_enum! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum TraceType {
        Debug,
        DebugPretty,
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = PROGRAM_NAME, about = "An experimental JIT compiler for OCaml bytecode")]
pub struct Options {
    /// Use the JIT compiler
    #[structopt(short = "j", long)]
    pub use_jit: bool,

    /// Show a trace of every instruction executed
    #[structopt(short, long)]
    pub trace: bool,

    /// The trace format to use if tracing is enabled
    #[structopt(long, default_value = "Debug", possible_values = &TraceType::variants(), case_insensitive = true)]
    pub trace_format: TraceType,

    #[structopt(long)]
    pub save_compiled: bool,
}

impl Options {
    pub fn get_from_env() -> Options {
        let args = env::var(ENV_VAR_KEY).unwrap_or_else(|_| String::from(""));
        let mut arg_sections = shell_words::split(&args).unwrap_or_else(|e| {
            eprintln!("Could not parse {} options!", ENV_VAR_KEY);
            eprintln!("Error: {:?}", e);
            Vec::new()
        });
        arg_sections.insert(0, String::from(PROGRAM_NAME));

        Options::from_iter(arg_sections.iter())
    }
}
