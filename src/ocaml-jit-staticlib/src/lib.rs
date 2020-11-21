// There's lots of unsafe stuff where the safety doc would just be "we're trusting OCaml does the
// right thing" or "we're trusting the JIT isn't broken"
#![allow(clippy::missing_safety_doc)]

// A side-effect of how we emit the JIT code means we have to cast functions to i64s not usizes
// It's fine here
#[allow(clippy::fn_to_numeric_cast)]
mod c_entrypoints;
mod caml;
mod compiler;
mod configuration;
mod global_data;
mod trace;

use crate::caml::mlvalues::LongValue;
use crate::compiler::{
    compile, compile_callback_if_needed, get_entrypoint, EntryPoint, LongjmpEntryPoint,
};
use crate::trace::{print_trace, PrintTraceType};
use caml::mlvalues::Value;
use global_data::GlobalData;
use std::ffi::c_void;

/* These are the hook points from the existing runtime to the JIT */

pub fn on_bytecode_loaded(code: &[i32]) -> *const c_void {
    let mut global_data = GlobalData::get();

    let print_traces = global_data.options.trace;

    if global_data.options.should_compile_code() {
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
        )
    } else {
        code.as_ptr() as *const c_void
    }
}

extern "C" {
    fn jit_support_main_wrapper(
        entrypoint: EntryPoint,
        longjmp_handler: LongjmpEntryPoint,
    ) -> Value;

    fn actual_caml_interprete(prog: *const i32, prog_size: usize, print_traces: bool) -> Value;

    static caml_callback_code: [i32; 7];
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    let mut global_data = GlobalData::get();
    let use_jit = global_data.options.use_jit;
    let print_traces = global_data.options.trace;

    if (use_jit || print_traces) && code.as_ptr() == unsafe { caml_callback_code.as_ptr() } {
        compile_callback_if_needed(&mut global_data.compiler_data, code, print_traces);
    }

    if use_jit {
        if code.is_empty() {
            // It's initialising, do nothing
            return LongValue::UNIT.into();
        }

        let entrypoint = get_entrypoint(&global_data.compiler_data, code);
        // explicitly release the mutex, as the interpreter sometimes calls into itself recursively
        // and we don't want to be holding it when that happens
        let longjmp_handler = global_data.compiler_data.get_longjmp_handler();
        std::mem::drop(global_data);

        unsafe { jit_support_main_wrapper(entrypoint, longjmp_handler) }
    } else {
        std::mem::drop(global_data);

        unsafe { actual_caml_interprete(code.as_ptr(), code.len(), print_traces) }
    }
}

pub fn on_bytecode_released(code: &[i32]) {
    let mut global_data = GlobalData::get();
    if global_data.options.should_compile_code() {
        global_data.compiler_data.release_section(code);
    }
}

pub fn old_interpreter_trace(
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let global_data = GlobalData::get();

    print_trace(
        &global_data,
        PrintTraceType::BytecodePC(pc),
        accu,
        env,
        extra_args,
        sp,
    );
}
