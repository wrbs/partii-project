use crate::caml::domain_state::{get_stack_high, get_trap_sp_addr};
use crate::caml::mlvalues::{Value, ValueType};
use crate::compiler::CompilerData;
use crate::global_data::GlobalData;
use ocaml_jit_shared::{Instruction, Opcode};

pub fn print_bytecode_trace(
    global_data: &GlobalData,
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let (section, pc_offset) = global_data
        .compiler_data
        .translate_bytecode_address(pc as usize)
        .expect("Could not find bytecode offset for PC");

    let opcode_val = unsafe { *pc };
    let opcode = Opcode::from_i32(opcode_val).expect("Invalid opcode");
    trace(
        global_data,
        format!("!T! PC = <{}; {}> {:?}", section, pc_offset, opcode).as_str(),
        accu,
        env,
        extra_args,
        sp,
    );
}

pub fn print_instruction_trace(
    global_data: &GlobalData,
    instruction: &Instruction<usize>,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    trace(
        global_data,
        format!("      - {:?}", instruction).as_str(),
        accu,
        env,
        extra_args,
        sp,
    );
}

const STACK_ELEMENTS_TO_SHOW: usize = 5;

fn trace(
    global_data: &GlobalData,
    start: &str,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let compiler_data = &global_data.compiler_data;

    let stack_high = get_stack_high();

    let stack_size = (stack_high as usize - sp as usize) / 8;

    let mut on_stack = String::new();
    for i in 0..stack_size.min(STACK_ELEMENTS_TO_SHOW) {
        unsafe {
            let val = *sp.offset(i as isize);
            if i > 0 {
                on_stack.push_str(", ");
            }
            on_stack.push_str(display_value(compiler_data, val).as_str());
        }
    }

    if stack_size > STACK_ELEMENTS_TO_SHOW {
        on_stack.push_str(", ...");
    }

    let tsp = unsafe { *get_trap_sp_addr() };

    println!(
        "{:<40}  ACCU={} ENV={:016X} E_A={:<3} SP={:<3} TSP={:016X} TOS={}",
        start,
        display_value(compiler_data, Value::from(accu as i64)),
        env,
        extra_args,
        stack_size,
        tsp,
        on_stack
    );
}

fn display_value(compiler_data: &CompilerData, value: Value) -> String {
    // In most cases it just shows the value as a 64 bit number;

    if let ValueType::Block(v) = value.decode_type() {
        let address = v.0 as usize;
        if let Some((section_number, offset_pc)) = compiler_data.translate_bytecode_address(address)
        {
            return format!("@{:>15}", format!("<{}; {}>", section_number, offset_pc));
        }
    }

    return format!("{:016X}", value.0 as u64);
}
