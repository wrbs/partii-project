use crate::caml::domain_state::get_stack_high;
use crate::caml::mlvalues::Value;
use ocaml_jit_shared::Instruction;

pub fn print_bytecode_trace(pc: usize, accu: u64, env: u64, extra_args: u64, sp: *const Value) {
    trace(format!("# PC = {}", pc).as_str(), accu, env, extra_args, sp);
}

pub fn print_instruction_trace(
    instruction: &Instruction<usize>,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    trace(
        format!("  - {:?}", instruction).as_str(),
        accu,
        env,
        extra_args,
        sp,
    );
}

fn trace(start: &str, accu: u64, env: u64, extra_args: u64, sp: *const Value) {
    let top_of_stack = unsafe { *sp };
    let stack_high = get_stack_high();

    let stack_size = (stack_high as usize - sp as usize) / 8;
    println!(
        "{:<30}  ACCU={:016X} ENV={:016X} E_A={:<3} SP={:<3} TOS={:016X}",
        start, accu, env, extra_args, stack_size, top_of_stack.0 as u64
    );
}
