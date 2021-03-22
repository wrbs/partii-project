use std::{ffi::CStr, os::raw::c_char};

use super::c_primitives::caml_fatal_error;
use crate::{
    caml::mlvalues::Value,
    global_data::GlobalData,
    trace::{print_trace, PrintTraceType},
};

pub extern "C" fn fatal_message(message: *const c_char) {
    unsafe {
        caml_fatal_error(message);
    }
}

pub extern "C" fn event_trace(
    message: *const c_char,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let s = unsafe { CStr::from_ptr(message).to_string_lossy() };

    let mut global_data = GlobalData::get();
    print_trace(
        &mut global_data,
        PrintTraceType::Event(&s),
        accu,
        env,
        extra_args,
        sp,
    );
}

pub extern "C" fn bytecode_trace(
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let mut global_data = GlobalData::get();
    print_trace(
        &mut global_data,
        PrintTraceType::BytecodePC(pc),
        accu,
        env,
        extra_args,
        sp,
    );
}

pub extern "C" fn instruction_trace(
    pc: i64,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
    section_number: u64,
) {
    let mut global_data = GlobalData::get();
    let section = global_data.compiler_data.sections[section_number as usize]
        .as_ref()
        .expect("Section already released");
    let instruction = &section.instructions.as_ref().unwrap()[pc as usize].clone();
    print_trace(
        &mut global_data,
        PrintTraceType::Instruction(instruction),
        accu,
        env,
        extra_args,
        sp,
    );
}
