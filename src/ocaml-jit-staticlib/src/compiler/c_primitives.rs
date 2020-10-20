extern "C" {
    pub fn jit_support_get_field(base: i64, field: i64) -> i64;
    pub fn jit_support_set_field(base: i64, field: i64, value: i64);
    pub fn jit_support_check_stacks(sp: i64) -> i64;
    pub fn jit_support_appterm_stacks(nargs: i64, slotsize: i64, sp: i64) -> i64;
    pub fn jit_support_closure(state: i64, nvars: i64, codeval: i64);
    pub fn jit_support_closure_rec(state: i64, nargs: i64, codeval: i64);
    pub fn jit_support_get_initial_sp() -> i64;
    pub fn jit_support_make_block(state: i64, wosize: i64, tag: i64);
    pub fn jit_support_get_primitive(primno: i64) -> i64;

    #[link_name = "caml_global_data"]
    static CAML_GLOBAL_DATA: i64;
}

#[inline(always)]
pub fn get_global_data_addr() -> i64 {
    let loc: *const i64 = unsafe { &CAML_GLOBAL_DATA };
    loc as i64
}
