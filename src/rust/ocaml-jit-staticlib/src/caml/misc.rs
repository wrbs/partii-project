use std::{ffi::CString, os::raw::c_char};

extern "C" {
    fn caml_fatal_error(s: *const c_char, ...) -> !;
}

pub fn fatal_error(message: &str) -> ! {
    let msg = CString::new(message).unwrap();
    unsafe { caml_fatal_error(msg.as_ptr()) }
}

/*
#[repr(C)]
pub struct ExtTable {
    size: i32,
    capacity: i32,
    contents: *const *const c_void,
}

impl ExtTable {
    pub fn get(&self, index: usize) -> *const c_void {
        unsafe { *self.contents.add(index) }
    }
}

 */
