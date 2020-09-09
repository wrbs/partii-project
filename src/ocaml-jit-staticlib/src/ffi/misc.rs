use std::os::raw::c_char;
use std::ffi::CString;
use std::panic;

extern "C" {
    fn caml_fatal_error(s: *const c_char, ...) -> !;
}

pub fn fatal_error(message: &str) -> !{
    let msg = CString:: new(message).unwrap();
    unsafe { caml_fatal_error(msg.as_ptr()) }
}
