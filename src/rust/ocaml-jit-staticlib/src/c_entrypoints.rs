use std::panic;
use std::slice;

use crate::caml::misc::fatal_error;
use crate::caml::mlvalues::Value;
use crate::{
    interpret_bytecode, old_interpreter_trace, on_bytecode_loaded, on_bytecode_released,
    on_shutdown,
};
use std::ffi::c_void;

// We need some way to convince Rust that the OCaml interpreter is single threaded
// Easiest way is to just use a mutex at each entry point for our global data

#[no_mangle]
pub extern "C" fn ocaml_jit_on_startup() {
    // Set up the panic hook to call into the OCaml fatal error machinery
    // this won't unwind but this doesn't seem to work anyway the way things are linked
    panic::set_hook(Box::new(|p| {
        fatal_error(format!("{}", p).as_str());
    }))
}

#[no_mangle]
pub unsafe extern "C" fn caml_interprete(prog: *const i32, prog_size: usize) -> Value {
    debug_assert!(prog_size % 4 == 0);
    let slice = slice::from_raw_parts(prog, prog_size / 4);
    interpret_bytecode(slice)
}

#[no_mangle]
pub unsafe extern "C" fn caml_prepare_bytecode(
    prog: *const i32,
    prog_size: usize,
) -> *const c_void {
    debug_assert!(prog_size % 4 == 0);
    let slice = slice::from_raw_parts(prog, prog_size / 4);
    on_bytecode_loaded(slice)
}

#[no_mangle]
pub unsafe extern "C" fn caml_release_bytecode(prog: *const i32, prog_size: usize) {
    debug_assert!(prog_size % 4 == 0);
    let slice = slice::from_raw_parts(prog, prog_size / 4);
    on_bytecode_released(slice);
}

#[no_mangle]
pub unsafe extern "C" fn rust_jit_trace(
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    old_interpreter_trace(pc, accu, env, extra_args, sp);
}

#[no_mangle]
pub extern "C" fn rust_jit_at_shutdown() {
    on_shutdown();
}
