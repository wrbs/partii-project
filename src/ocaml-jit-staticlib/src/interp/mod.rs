use crate::caml::mlvalues::Value;
use std::sync::{Mutex, MutexGuard};

mod code_preparation;
mod global_data;
mod compiler;

use global_data::GlobalData;
use ocaml_jit_shared::Opcode;

pub fn on_bytecode_loaded(code: &[i32]) {
    let mut global_data = GlobalData::get();
    code_preparation::relocate_and_load_instructions(code);
}

extern "C" {
    fn actual_caml_interprete(prog: *const i32, prog_size: usize) -> Value;
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    unsafe { actual_caml_interprete(code.as_ptr(), code.len()) }
}

pub fn on_bytecode_released(_code: &[i32]) {
    // For now, don't cleanup and just leak memory
}

pub fn trace(pc: *const i32, sp: u64, acc: i64) {
    /*let global_data = GlobalData::get();
    if !global_data.trace {
        return;
    }

    let opcode = unsafe { Opcode::from_i32(*pc).expect("Invalid opcode") };

    let (start, number_of_instructions) = global_data
        .lookup
        .lookup(pc)
        .expect("Could not find instruction for pc");

    print!(
        "PC={:5} SP={:5} ACCU={:16X}\t{} {:?}",
        start,
        sp,
        acc as u64,
        opcode.ocaml_name(),
        global_data.instructions[start]
    );
    for instruction_index in start + 1..=start + number_of_instructions {
        print!(", {:?}", global_data.instructions[instruction_index]);
    }
    println!();*/
}
