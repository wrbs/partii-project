use std::{ffi::CStr, os::raw::c_char};

use ocaml_jit_shared::{call_trace::CallTrace, BytecodeLocation, BytecodeRelativeOffset};

use super::{c_primitives::caml_fatal_error, emit_code::ClosureMetadataTableEntry};
use crate::{
    caml::mlvalues::Value,
    configuration::CraneliftErrorHandling,
    global_data::GlobalData,
    trace::{print_call_trace, print_instruction_trace, PrintTraceType},
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
    print_instruction_trace(
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
    print_instruction_trace(
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
    print_instruction_trace(
        &mut global_data,
        PrintTraceType::Instruction(instruction),
        accu,
        env,
        extra_args,
        sp,
    );
}

pub extern "C" fn compile_closure_optimised(closure: *mut ClosureMetadataTableEntry) {
    let closure = unsafe { closure.as_mut().unwrap() };
    let section_number = closure.section as usize;
    let entrypoint = closure.bytecode_offset as usize;
    let arity = closure.required_extra_args + 1;
    if arity > 5 {
        closure.execution_count_status = -3; // Error, don't try again
        return;
    }

    let mut global_data = GlobalData::get();
    let section = global_data.compiler_data.sections[closure.section as usize]
        .as_ref()
        .expect("No such section");
    let code = unsafe { section.get_code() };

    let error_handling = global_data.options.cranelift_error_handling;

    let GlobalData {
        compiler_data,
        optimised_compiler,
        ..
    } = &mut *global_data;
    match optimised_compiler.optimise_closure(section_number, code, entrypoint, compiler_data) {
        Ok(Some(new_code)) => {
            closure.compiled_location = new_code as u64;
            closure.execution_count_status = -2; // optimised
        }
        Ok(None) => {
            // This means the closure isn't supported by the compiler but in an expected way
            closure.execution_count_status = -3; // Error, don't try again
        }
        Err(e) => {
            match error_handling {
                CraneliftErrorHandling::Ignore => {}
                CraneliftErrorHandling::Log => {
                    eprintln!("{:?}", e);
                }
                CraneliftErrorHandling::Panic => {
                    eprintln!("{:?}", e);
                    panic!("Problem compiling closure");
                }
            }
            closure.execution_count_status = -3; // Error, tells apply not to try again
        }
    }
}

pub extern "C" fn emit_enter_apply_trace(
    closure: *const ClosureMetadataTableEntry,
    sp: *const u64,
    extra_args: usize,
) {
    let nargs = extra_args + 1;
    let args = (0..nargs).map(|i| unsafe { *(sp.add(i)) }).collect();
    do_call_trace(CallTrace::Enter {
        closure: get_bytecode_location_from_closure(closure),
        needed: get_arity_from_closure(closure),
        provided: args,
    });
}

pub extern "C" fn emit_return_trace(retval: u64) {
    do_call_trace(CallTrace::Return(retval));
}

fn do_call_trace(trace: CallTrace) {
    let global_data = GlobalData::get();
    let trace_format = global_data.options.trace_format;

    print_call_trace(&trace, &trace_format)
}

fn get_bytecode_location_from_closure(
    closure: *const ClosureMetadataTableEntry,
) -> BytecodeLocation {
    let closure = unsafe { closure.as_ref().unwrap() };
    let section_number = closure.section as usize;
    let entrypoint = closure.bytecode_offset as usize;

    BytecodeLocation {
        section_number,
        offset: BytecodeRelativeOffset(entrypoint),
    }
}

fn get_arity_from_closure(closure: *const ClosureMetadataTableEntry) -> usize {
    let closure = unsafe { closure.as_ref().unwrap() };
    closure.required_extra_args as usize + 1
}

pub extern "C" fn emit_c_call_trace(id: u32, sp: *const u64, nargs: usize) {
    let args = (0..nargs).map(|i| unsafe { *(sp.add(i)) }).collect();
    do_call_trace(CallTrace::CCall {
        id: id as usize,
        args,
    });
}

fn do_custom_trace(message: String) {
    do_call_trace(CallTrace::Custom(message));
}

pub unsafe extern "C" fn make_block_trace(loc: u64) {
    let header = *(loc as *const u64).offset(-1);
    do_custom_trace(format!("Make block: header={:#x} loc={:#x}", header, loc));
}
