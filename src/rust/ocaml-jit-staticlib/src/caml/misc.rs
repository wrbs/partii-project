use std::{ffi::CString, os::raw::c_char};

extern "C" {
    fn caml_fatal_error(s: *const c_char, ...) -> !;
}

pub fn fatal_error(message: &str) -> ! {
    let msg = CString::new(message).unwrap();
    unsafe { caml_fatal_error(msg.as_ptr()) }
}

#[repr(C)]
pub struct ExtTable {
    pub size: i32,
    pub capacity: i32,
    pub contents: *const usize,
}

impl ExtTable {
    pub fn get(&self, index: usize) -> usize {
        unsafe { *self.contents.add(index) }
    }

    pub fn as_slice(&self) -> &[usize] {
        unsafe { std::slice::from_raw_parts(self.contents, self.size as usize) }
    }
}

extern "C" {
    #[link_name = "caml_prim_table"]
    pub static CAML_PRIMITIVE_TABLE: ExtTable;
}
