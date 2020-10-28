use crate::caml::domain_state::{get_stack_high, get_trap_sp};
use crate::caml::mlvalues::{Value, ValueType};
use crate::compiler::CompilerData;
use crate::configuration::TraceType;
use crate::global_data::GlobalData;
use ocaml_jit_shared::{
    BytecodeRelativeOffset, Instruction, Opcode, TraceEntry, TraceLocation, ValueOrBytecodeLocation,
};

const STACK_ELEMENTS_TO_SHOW: usize = 5;

pub fn print_bytecode_trace(
    global_data: &GlobalData,
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let trace = get_bytecode_trace(global_data, pc, accu, env, extra_args, sp);
    print_trace(global_data.options.trace_format, &trace);
}

pub fn print_instruction_trace(
    global_data: &GlobalData,
    instruction: &Instruction<BytecodeRelativeOffset>,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let trace = get_instruction_trace(global_data, instruction, accu, env, extra_args, sp);
    print_trace(global_data.options.trace_format, &trace);
}

fn get_bytecode_trace(
    global_data: &GlobalData,
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) -> TraceEntry {
    let bytecode_loc = global_data
        .compiler_data
        .translate_bytecode_address(pc as usize)
        .expect("Could not find bytecode offset for PC");

    let opcode_val = unsafe { *pc };
    let opcode = Opcode::from_i32(opcode_val).expect("Invalid opcode");

    let location = TraceLocation::Bytecode {
        pc: bytecode_loc,
        opcode,
    };

    return get_trace(global_data, location, accu, env, extra_args, sp);
}

pub fn get_instruction_trace(
    global_data: &GlobalData,
    instruction: &Instruction<BytecodeRelativeOffset>,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) -> TraceEntry {
    get_trace(
        global_data,
        TraceLocation::ParsedInstruction(instruction.clone()),
        accu,
        env,
        extra_args,
        sp,
    )
}

fn get_trace(
    global_data: &GlobalData,
    location: TraceLocation,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) -> TraceEntry {
    let compiler_data = &global_data.compiler_data;

    let stack_high = get_stack_high();

    let stack_size = (stack_high as usize - sp as usize) / 8;

    let mut top_of_stack = Vec::new();
    for i in 0..stack_size.min(STACK_ELEMENTS_TO_SHOW) {
        unsafe {
            let val = *sp.offset(i as isize);
            top_of_stack.push(process_value(compiler_data, val));
        }
    }

    let trap_sp = get_trap_sp();

    return TraceEntry {
        location,
        accu: process_value(compiler_data, Value(accu as i64)),
        env,
        extra_args,
        sp: sp as u64,
        trap_sp,
        stack_size,
        top_of_stack,
    };
}

fn process_value(compiler_data: &CompilerData, value: Value) -> ValueOrBytecodeLocation {
    // In most cases it just shows the value as a 64 bit number;

    if let ValueType::Block(v) = value.decode_type() {
        let address = v.0 as usize;
        if let Some(loc) = compiler_data.translate_bytecode_address(address) {
            return ValueOrBytecodeLocation::BytecodeLocation(loc);
        }
    }

    ValueOrBytecodeLocation::Value(value.0 as u64)
}

fn print_trace(trace_format: TraceType, trace: &TraceEntry) {
    match trace_format {
        TraceType::Debug => println!("{:?}", trace),
        TraceType::DebugPretty => println!("{:#?}", trace),
    }
}
