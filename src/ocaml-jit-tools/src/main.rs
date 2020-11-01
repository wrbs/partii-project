mod bytecode_files;
mod commands;
mod utils;

use commands::{compare_traces, disassembler, hexdump, process_disassembly};

#[macro_use]
extern crate prettytable;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    name = "ocaml-jit-tools",
    about = "additional tools to help in developing OCaml JIT"
)]
enum BaseCli {
    Dis(disassembler::Options),
    Hexdump(hexdump::Options),
    CompareTraces(compare_traces::Options),
    ProcessDisassembly(process_disassembly::Options),
}

fn main() {
    setup_pipes();

    let subcommand = BaseCli::from_args();
    match subcommand {
        BaseCli::Dis(opts) => disassembler::run(opts),
        BaseCli::Hexdump(opts) => hexdump::run(opts),
        BaseCli::CompareTraces(opts) => compare_traces::run(opts),
        BaseCli::ProcessDisassembly(opts) => process_disassembly::run(opts),
    }
}

// stop broken pipe errors for these tools
fn setup_pipes() {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;

        unsafe {
            if let Err(e) = signal::signal(signal::SIGPIPE, signal::SigHandler::SigDfl) {
                eprintln!("Error setting up sigpipe handler: {}", e.to_string());
            }
        }
    }
}
