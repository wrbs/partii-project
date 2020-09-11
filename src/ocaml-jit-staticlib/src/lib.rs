// There's lots of unsafe stuff where the safety doc would just be "we're trusting OCaml does the
// right thing" or "we're trusting the JIT isn't broken"
#![allow(clippy::missing_safety_doc)]

use std::panic;
use std::slice;

mod caml;
mod interp;

use caml::misc::fatal_error;
use caml::mlvalues::Value;

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
    let slice = slice::from_raw_parts(prog, prog_size);
    interp::interpret_bytecode(slice)
}

#[no_mangle]
pub unsafe extern "C" fn caml_prepare_bytecode(prog: *const i32, prog_size: usize) {
    let slice = slice::from_raw_parts(prog, prog_size);
    interp::on_bytecode_loaded(slice);
}

#[no_mangle]
pub unsafe extern "C" fn caml_release_bytecode(prog: *const i32, prog_size: usize) {
    let slice = slice::from_raw_parts(prog, prog_size);
    interp::on_bytecode_released(slice);
}
