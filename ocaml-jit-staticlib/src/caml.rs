/*
 * Low-level rust wrappers for the OCaml headers
 */

use std::os::raw;

#[allow(non_camel_case_types)]
pub type char_os = raw::c_char;


extern "C" {
    #[link_name = "caml_byt_main"]
    pub fn byt_main(args: *mut *mut char_os);
}
