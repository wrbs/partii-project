mod bytecode_files;
mod commands;

use anyhow::{Context, Result};
use commands::{clever_dis, compare_traces, disassembler, hexdump, process_disassembly};

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
    CleverDis(clever_dis::Options),
}

fn main() -> Result<()> {
    setup_pipes()?;

    let subcommand = BaseCli::from_args();
    match subcommand {
        BaseCli::Dis(opts) => disassembler::run(opts),
        BaseCli::Hexdump(opts) => hexdump::run(opts),
        BaseCli::CompareTraces(opts) => compare_traces::run(opts),
        BaseCli::ProcessDisassembly(opts) => process_disassembly::run(opts),
        BaseCli::CleverDis(opts) => clever_dis::run(opts),
    }
}

// stop broken pipe errors for these tools
fn setup_pipes() -> Result<()> {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;

        unsafe {
            signal::signal(signal::SIGPIPE, signal::SigHandler::SigDfl)
                .context("Failed to set up broken pipe handler")?;
        }
    }

    Ok(())
}
