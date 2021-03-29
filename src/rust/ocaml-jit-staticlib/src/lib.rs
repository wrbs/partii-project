// There's lots of unsafe stuff where the safety doc would just be "we're trusting OCaml does the
// right thing" or "we're trusting the JIT isn't broken"
#![allow(clippy::missing_safety_doc, clippy::clippy::fn_to_numeric_cast)]

use std::{ffi::c_void, fs, io::Write};

use caml::mlvalues::Value;
use compiler::PrintTraces;
use global_data::GlobalData;

use crate::{
    caml::mlvalues::LongValue,
    compiler::{
        compile, compile_callback_if_needed, get_entrypoint, EntryPoint, LongjmpEntryPoint, Section,
    },
    trace::{print_instruction_trace, PrintTraceType},
};

// A side-effect of how we emit the JIT code means we have to cast functions to i64s not usizes
// It's fine here
#[allow(clippy::fn_to_numeric_cast)]
mod c_entrypoints;
mod caml;
mod compiler;
mod configuration;
mod global_data;
mod trace;

/* These are the hook points from the existing runtime to the JIT */

pub fn on_startup() -> GlobalData {
    let global_data = GlobalData::new();

    if let Some(output_dir) = global_data.options.output_dir.as_ref() {
        fs::create_dir_all(output_dir).unwrap();
    }

    global_data
}

pub fn on_bytecode_loaded(code: &[i32]) -> *const c_void {
    let mut global_data = GlobalData::get();

    let compiler_options = global_data.compiler_data.compiler_options;

    if global_data.options.use_compiler {
        let section_number = compile(&mut global_data.compiler_data, code, compiler_options);
        let section = global_data.compiler_data.sections[section_number]
            .as_ref()
            .unwrap();
        on_section_compiled(&global_data, section);

        if global_data.options.use_jit {
            section.first_instruction_location as *const c_void
        } else {
            code.as_ptr() as *const c_void
        }
    } else {
        code.as_ptr() as *const c_void
    }
}

fn on_section_compiled(global_data: &GlobalData, section: &Section) {
    if global_data.options.save_compiled {
        let path = global_data
            .options
            .output_path(format!("section{}.bin", section.section_number));
        fs::write(path, &*section.compiled_code).unwrap();
    }
}

extern "C" {
    fn jit_support_main_wrapper(
        entrypoint: EntryPoint,
        longjmp_handler: LongjmpEntryPoint,
    ) -> Value;

    fn actual_caml_interprete(prog: *const i32, prog_size: usize, print_traces: bool) -> Value;

    static caml_callback_code: [i32; 7];
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    let mut global_data = GlobalData::get();
    let use_jit = global_data.options.use_jit;
    let compiler_options = global_data.compiler_data.compiler_options;
    let print_traces = compiler_options.print_traces;

    if (use_jit || print_traces == Some(PrintTraces::Instruction))
        && code.as_ptr() == unsafe { caml_callback_code.as_ptr() }
    {
        let maybe_section =
            compile_callback_if_needed(&mut global_data.compiler_data, code, compiler_options);
        if let Some(section_number) = maybe_section {
            let section = global_data.compiler_data.sections[section_number]
                .as_ref()
                .unwrap();
            on_section_compiled(&global_data, section);
        }
    }

    if use_jit {
        if code.is_empty() {
            // It's initialising, do nothing
            return LongValue::UNIT.into();
        }

        let entrypoint = get_entrypoint(&global_data.compiler_data, code);
        // explicitly release the mutex, as the interpreter sometimes calls into itself recursively
        // and we don't want to be holding it when that happens
        let longjmp_handler = global_data.compiler_data.get_longjmp_handler();
        std::mem::drop(global_data);

        unsafe { jit_support_main_wrapper(entrypoint, longjmp_handler) }
    } else {
        std::mem::drop(global_data);

        let instruction_traces = print_traces == Some(PrintTraces::Instruction);
        unsafe { actual_caml_interprete(code.as_ptr(), code.len(), instruction_traces) }
    }
}

pub fn on_bytecode_released(code: &[i32]) {
    let mut global_data = GlobalData::get();
    if global_data.options.use_compiler {
        global_data.compiler_data.release_section(code);
    }
}

pub fn old_interpreter_trace(
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

pub fn on_shutdown() {
    let global_data = GlobalData::get();
    if global_data.options.save_instruction_counts {
        let instruction_counts = global_data.instruction_counts.as_ref().unwrap();

        let json_path = global_data.options.output_path("instruction_counts.json");
        let json = serde_json::to_string_pretty(instruction_counts).unwrap();
        fs::write(json_path, json).unwrap();

        let total_instrs = instruction_counts.values().sum::<usize>() as f32;
        let mut counts: Vec<_> = instruction_counts.iter().collect();
        counts.sort_by_key(|(_, c)| **c);
        counts.reverse();

        let path = global_data.options.output_path("instruction_counts");
        let mut f = fs::File::create(path).unwrap();

        for (opcode, count) in counts {
            writeln!(
                f,
                "{:?}: {:.2}% ({})",
                opcode,
                (*count as f32) / total_instrs * 100.0,
                count
            )
            .unwrap();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_jit_lookup_stack_maps(
    ip: *const u64,
    f: extern "C" fn(u64, *const u64),
) {
    let return_addr = *ip;

    let global_data = GlobalData::get();
    global_data
        .optimised_compiler
        .lookup_stack_map(return_addr, |offset| {
            let pointer = ip.add(offset);
            f(*pointer, pointer);
        });
}
