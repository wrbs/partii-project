/*
 *
 * Options are set by setting the JIT_OPTIONS environment variable to an argument string which
 * is parsed to avoid impacting the runtime and OCaml program\*/

use std::env;
use structopt::StructOpt;

const PROGRAM_NAME: &str = "ocaml-jit";
const ENV_VAR_KEY: &str = "JIT_OPTIONS";

#[derive(StructOpt)]
#[structopt(name = PROGRAM_NAME, about = "An experimental JIT compiler for OCaml bytecode")]
pub struct Options {
    #[structopt(short = "j", long)]
    pub use_jit: bool,

    #[structopt(short, long)]
    pub trace: bool,

    #[structopt(long)]
    pub save_compiled: bool,
}

pub fn get_options_from_env() -> Options {
    let args = env::var(ENV_VAR_KEY).unwrap_or_else(|_| String::from(""));
    let mut arg_sections = shell_words::split(&args).unwrap_or_else(|e| {
        eprintln!("Could not parse {} options!", ENV_VAR_KEY);
        eprintln!("Error: {:?}", e);
        Vec::new()
    });
    arg_sections.insert(0, String::from(PROGRAM_NAME));

    Options::from_iter(arg_sections.iter())
}
