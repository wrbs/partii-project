use std::panic;
use crate::ffi::misc::fatal_error;

mod ffi;
mod interp;

#[no_mangle]
pub extern "C" fn ocaml_jit_on_startup() {
    // Set up the panic hook to call into the OCaml fatal error machinery
    // this won't unwind but this doesn't seem to work anyway the way things are linked
    panic::set_hook(Box::new(|p| {
        fatal_error(format!("{}", p).as_str());
    }))
}

