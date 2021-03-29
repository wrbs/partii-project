use std::os::raw::c_char;

extern "C" {
    // OCaml existing primitives
    pub fn caml_modify(ptr: i64, value: i64);
    pub fn caml_process_pending_actions();
    pub fn caml_fatal_error(s: *const c_char, ...) -> !;

    // Our new ones
    pub fn jit_support_get_float_field(state: i64, field: i64) -> i64;
    pub fn jit_support_set_float_field(base: i64, field: i64, value: i64);

    pub fn jit_support_vect_length(ptr: i64) -> i64;

    pub fn jit_support_check_stacks(sp: i64) -> i64;
    pub fn jit_support_appterm_stacks(nargs: i64, slotsize: i64, sp: i64) -> i64;
    pub fn jit_support_closure(state: i64, nvars: i64, codeval: i64);
    pub fn jit_support_closure_rec(state: i64, nargs: i64, codeval: i64);

    pub fn jit_support_make_block(state: i64, wosize: i64, tag: i64);
    pub fn jit_support_make_float_block(state: i64, size: i64);

    pub fn jit_support_get_primitive(primno: i64) -> i64;

    pub fn caml_raise_zero_divide();

    pub fn jit_support_restart(state: i64);
    pub fn jit_support_grab_closure(state: i64, prev_restart: i64) -> i64;

    pub fn jit_support_stop(initial_state: i64, sp: i64);
    pub fn jit_support_raise_check(initial_state: i64) -> i64;

    pub fn jit_support_get_dyn_met(tag: i64, obj: i64) -> i64;

    #[link_name = "caml_global_data"]
    static CAML_GLOBAL_DATA: i64;

    #[link_name = "caml_something_to_do"]
    static CAML_SOMETHING_TO_DO: i64;
}

#[inline(always)]
pub fn get_global_data_addr() -> i64 {
    let loc: *const i64 = unsafe { &CAML_GLOBAL_DATA };
    loc as i64
}

#[inline(always)]
pub fn get_something_to_do_addr() -> i64 {
    let loc: *const i64 = unsafe { &CAML_SOMETHING_TO_DO };
    loc as i64
}
