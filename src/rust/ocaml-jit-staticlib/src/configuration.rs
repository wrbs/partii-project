/*
 *
 * Options are set by setting the JIT_OPTIONS environment variable to an argument string which
 * is parsed to avoid impacting the runtime and OCaml program\*/

use std::{
    env,
    path::{Path, PathBuf},
};

use clap::arg_enum;
use default_env::default_env;
use structopt::StructOpt;

const PROGRAM_NAME: &str = "ocaml-jit";
const ENV_VAR_KEY: &str = "JIT_OPTIONS";

const DEFAULT_JIT_OPTIONS: &str = default_env!("DEFAULT_JIT_OPTIONS", "");

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

arg_enum! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum CraneliftErrorHandling {
        Panic,
        Log,
        Ignore,
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = PROGRAM_NAME, about = "An experimental JIT compiler for OCaml bytecode")]
pub struct Options {
    /// Execute using the JIT compiled code
    #[structopt(short = "j", long)]
    pub use_jit: bool,

    /// Run the compiler at startup
    #[structopt(short = "c", long)]
    pub use_compiler: bool,

    /// Show a trace of every instruction executed
    #[structopt(short, long, conflicts_with = "call-trace")]
    pub trace: bool,

    #[structopt(short = "C", long, conflicts_with = "trace")]
    pub call_trace: bool,

    /// The trace format to use if tracing is enabled
    #[structopt(long, default_value = "Colorful", possible_values = &TraceType::variants(), case_insensitive = true)]
    pub trace_format: TraceType,

    /// The base directory to store artifacts from the execution
    #[structopt(short, long, parse(from_os_str))]
    pub output_dir: Option<PathBuf>,

    #[structopt(long, requires = "output-dir")]
    pub save_compiled: bool,

    #[structopt(long, requires_all = &["output-dir", "trace"])]
    pub save_instruction_counts: bool,

    #[structopt(long, conflicts_with = "no-hot-threshold")]
    pub hot_threshold: Option<usize>,

    #[structopt(long, conflicts_with = "hot-threshold")]
    pub no_hot_threshold: bool,

    #[structopt(long, default_value = "Panic")]
    pub cranelift_error_handling: CraneliftErrorHandling,
}

impl Options {
    pub fn get_from_env() -> Options {
        let args = env::var(ENV_VAR_KEY).unwrap_or_else(|_| String::from(DEFAULT_JIT_OPTIONS));
        let mut arg_sections = shell_words::split(&args).unwrap_or_else(|e| {
            eprintln!("Could not parse {} options!", ENV_VAR_KEY);
            eprintln!("Error: {:?}", e);
            Vec::new()
        });
        arg_sections.insert(0, String::from(PROGRAM_NAME));

        let mut opts = Options::from_iter(arg_sections.iter());
        opts.set_defaults();
        opts
    }

    fn set_defaults(&mut self) {
        if self.trace || self.use_jit {
            self.use_compiler = true
        }
    }

    pub fn output_path<P: AsRef<Path>>(&self, name: P) -> PathBuf {
        let base = self
            .output_dir
            .as_ref()
            .expect("No output directory specified!");
        base.join(name)
    }
}
