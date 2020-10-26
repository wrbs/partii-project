// There's lots of unsafe stuff where the safety doc would just be "we're trusting OCaml does the
// right thing" or "we're trusting the JIT isn't broken"
#![allow(clippy::missing_safety_doc)]

mod c_entrypoints;
mod caml;
mod compiler;
mod configuration;
mod global_data;
mod trace;

use crate::caml::mlvalues::LongValue;
use crate::compiler::{compile, get_entrypoint};
use crate::trace::print_bytecode_trace;
use caml::mlvalues::Value;
use global_data::GlobalData;

/* These are the hook points from the existing runtime to the JIT */

pub fn on_bytecode_loaded(code: &[i32]) {
    let mut global_data = GlobalData::get();

    let print_traces = global_data.options.trace;
    let write_code_to = if global_data.options.save_compiled {
        Some("/tmp/code")
    } else {
        None
    };

    compile(
        &mut global_data.compiler_data,
        code,
        print_traces,
        write_code_to,
    );
}

extern "C" {
    fn actual_caml_interprete(prog: *const i32, prog_size: usize, print_traces: bool) -> Value;
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    let global_data = GlobalData::get();
    let use_jit = global_data.options.use_jit;
    let print_traces = global_data.options.trace;

    if use_jit {
        if code.is_empty() {
            // It's initialising, do nothing
            return LongValue::UNIT.into();
        }

        let entrypoint = get_entrypoint(&global_data.compiler_data, code);
        // explicitly release the mutex, as the interpreter sometimes calls into itself recursively
        // and we don't want to be holding it when that happens
        std::mem::drop(global_data);

        entrypoint()
    } else {
        std::mem::drop(global_data);

        unsafe { actual_caml_interprete(code.as_ptr(), code.len(), print_traces) }
    }
}

pub fn on_bytecode_released(_code: &[i32]) {
    // For now, don't cleanup and just leak memory
}

pub fn old_interpreter_trace(pc: usize, accu: u64, env: u64, extra_args: u64, sp: *const Value) {
    let global_data = GlobalData::get();
    let (section, pc_offset) = global_data
        .compiler_data
        .translate_bytecode_address(pc)
        .expect("Could not find bytecode offset for PC");

    print_bytecode_trace(&global_data, section, pc_offset, accu, env, extra_args, sp);
}
