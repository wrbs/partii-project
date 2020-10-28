use crate::{BytecodeLocation, BytecodeRelativeOffset, Instruction, Opcode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ValueOrBytecodeLocation {
    Value(u64),
    BytecodeLocation(BytecodeLocation),
}

impl PartialEq for ValueOrBytecodeLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueOrBytecodeLocation::Value(a), ValueOrBytecodeLocation::Value(b)) => a == b,
            (
                ValueOrBytecodeLocation::BytecodeLocation(a),
                ValueOrBytecodeLocation::BytecodeLocation(b),
            ) => a == b,
            // This is for comparing the JIT and the bytecode interpreter's trace output
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum TraceLocation {
    Bytecode {
        pc: BytecodeLocation,
        opcode: Opcode,
    },
    ParsedInstruction(Instruction<BytecodeRelativeOffset>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TraceEntry {
    pub location: TraceLocation,
    pub accu: ValueOrBytecodeLocation,
    pub env: u64,
    pub extra_args: u64,
    pub sp: u64,
    pub trap_sp: u64,
    pub stack_size: usize,
    pub top_of_stack: Vec<ValueOrBytecodeLocation>,
}
