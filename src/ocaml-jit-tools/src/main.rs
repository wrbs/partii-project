mod bytecode_files;
mod commands;
mod utils;

use commands::{disassembler, hexdump};

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
}

fn main() {
    setup_pipes();

    let subcommand = BaseCli::from_args();
    match subcommand {
        BaseCli::Dis(opts) => disassembler::run(opts),
        BaseCli::Hexdump(opts) => hexdump::run(opts),
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
