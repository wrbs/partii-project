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
        Colorful,
        Plain,
        JSON,
        Debug,
        DebugPretty,
        NoPrint,
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = PROGRAM_NAME, about = "An experimental JIT compiler for OCaml bytecode")]
pub struct Options {
    /// Execute using the JIT compiled code
    #[structopt(short = "j", long)]
    pub use_jit: bool,

    /// Run the compiler at startup (--use-jit will automatically set this)
    #[structopt(short = "c", long)]
    pub use_compiler: bool,

    /// Show a trace of every instruction executed
    #[structopt(short, long)]
    pub trace: bool,

    /// The trace format to use if tracing is enabled
    #[structopt(long, default_value = "Colorful", possible_values = &TraceType::variants(), case_insensitive = true)]
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

    pub fn should_compile_code(&self) -> bool {
        self.use_compiler || self.use_jit || self.trace
    }
}
