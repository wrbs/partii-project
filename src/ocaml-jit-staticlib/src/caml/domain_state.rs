use super::mlvalues::Value;
use std::os::raw::c_void;

// Using void pointers where I don't care about the types

#[repr(C, align(8))]
pub struct DomainState {
    young_ptr: *const c_void,
    young_limit: *const c_void,

    exception_pointer: *const c_void,

    young_base: *const c_void,
    young_start: *const c_void,
    young_end: *const c_void,
    young_alloc_start: *const c_void,
    young_alloc_end: *const c_void,
    young_alloc_mid: *const c_void,
    young_trigger: *const c_void,
    minor_heap_wsz: usize,
    in_minor_collection: i64,
    extra_heap_resources_minor: f64,
    ref_table: *const c_void,
    ephe_ref_table: *const c_void,
    custom_table: *const c_void,

    stack_low: *const Value,
    stack_high: *const Value,
    stack_threshold: *const Value,
    extern_sp: u64,
    trapsp: u64,
    trap_barrier: *mut Value,
    external_raise: u64, // todo - sigjmp_buf, get _JBLEN
    exn_bucket: Value,

    top_of_stack: *const c_void,
    bottom_of_stack: *const c_void,
    last_return_address: u64,
    gc_regs: *const c_void,

    backtrace_active: i64,
    backtrace_pos: i64,
    backtrace_buffer: *const c_void,
    backtrace_last_exn: Value,

    compare_unordered: i64,
    requested_major_slice: i64,
    requested_minor_gc: i64,
    local_roots: u64,

    stat_minor_words: f64,
    stat_promoted_words: f64,
    stat_major_words: f64,
    stat_minor_collections: i64,
    stat_major_collections: i64,
    stat_heap_wsz: i64,
    stat_top_heap_wsz: i64,
    stat_compactions: i64,
    stat_heap_chunks: i64,

    eventlog_startup_timestamp: u64,
    eventlog_startup_pid: u32,
    eventlog_paused: u64,
    eventlog_enabled: u64,
    eventlog_out: *const c_void,
}

extern "C" {
    static Caml_state: *mut DomainState;
}

/*
pub fn get_sp() -> *mut Value {
    unsafe { (*Caml_state).extern_sp }
}

pub fn set_sp(to: *mut Value) {
    unsafe { (*Caml_state).extern_sp = to }
}

pub fn stack_size() -> i64 {
    unsafe { ((*Caml_state).stack_high as i64 - (*Caml_state).extern_sp as i64) / 8 }
}
 */

pub fn get_stack_high_addr() -> *const (*const Value) {
    unsafe { &(*Caml_state).stack_high }
}

pub fn get_stack_high() -> *const Value {
    unsafe { *get_stack_high_addr() }
}

pub fn get_trap_sp_addr() -> *const u64 {
    unsafe { &(*Caml_state).trapsp }
}

pub fn get_trap_sp() -> u64 {
    unsafe { *get_trap_sp_addr() }
}

pub fn get_local_roots_addr() -> *const u64 {
    unsafe { &(*Caml_state).local_roots }
}
pub fn get_extern_sp_addr() -> *const u64 {
    unsafe { &(*Caml_state).extern_sp }
}
pub fn get_external_raise_addr() -> *const u64 {
    unsafe { &(*Caml_state).external_raise }
}
