use ocaml_jit_shared::{
    BytecodeRelativeOffset, Instruction, Opcode, TraceEntry, TraceLocation, ValueOrBytecodeLocation,
};

use crate::caml::domain_state::{get_stack_high, get_trap_sp};
use crate::caml::mlvalues::{Value, ValueType};
use crate::compiler::CompilerData;
use crate::configuration::TraceType;
use crate::global_data::GlobalData;

const STACK_ELEMENTS_TO_SHOW: usize = 5;

pub enum PrintTraceType<'a> {
    BytecodePC(*const i32),
    Instruction(&'a Instruction<BytecodeRelativeOffset>),
    Event(&'a str),
}

pub fn print_trace(
    global_data: &mut GlobalData,
    trace_type: PrintTraceType,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    if global_data.options.save_instruction_counts {
        if let PrintTraceType::BytecodePC(pc) = trace_type {
            let opcode_val = unsafe { *pc };
            let opcode = Opcode::from_i32(opcode_val).expect("Invalid opcode");

            let instruction_counts = global_data.instruction_counts.as_mut().unwrap();
            let count = instruction_counts.entry(opcode).or_insert(0);
            *count += 1;
        }
    }

    let trace_format = global_data.options.trace_format;
    if trace_format == TraceType::NoPrint {
        return;
    }

    let trace = get_trace(global_data, trace_type, accu, env, extra_args, sp);

    match trace_format {
        TraceType::Colorful => trace.print_colored(),
        TraceType::Plain => trace.print(),
        TraceType::JSON => println!("!T! {}", serde_json::to_string(&trace).unwrap()),
        TraceType::Debug => println!("{:?}", &trace),
        TraceType::DebugPretty => println!("{:#?}", &trace),
        TraceType::NoPrint => unreachable!(),
    }
}

fn get_trace(
    global_data: &GlobalData,
    trace_type: PrintTraceType,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) -> TraceEntry {
    let location = match trace_type {
        PrintTraceType::BytecodePC(pc) => {
            let bytecode_loc = global_data
                .compiler_data
                .translate_bytecode_address(pc as usize)
                .expect("Could not find bytecode offset for PC");

            let opcode_val = unsafe { *pc };
            let opcode = Opcode::from_i32(opcode_val).expect("Invalid opcode");

            TraceLocation::Bytecode {
                pc: bytecode_loc,
                opcode,
            }
        }
        PrintTraceType::Instruction(i) => TraceLocation::ParsedInstruction(i.clone()),
        PrintTraceType::Event(s) => TraceLocation::Event(String::from(s)),
    };

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

    TraceEntry {
        location,
        accu: process_value(compiler_data, Value(accu as i64)),
        env,
        extra_args,
        sp: sp as u64,
        trap_sp,
        stack_size,
        top_of_stack,
    }
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
