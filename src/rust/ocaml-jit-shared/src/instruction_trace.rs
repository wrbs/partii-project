use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::{BytecodeLocation, BytecodeRelativeOffset, Instruction, Opcode};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum ValueOrBytecodeLocation {
    Value(u64),
    BytecodeLocation(BytecodeLocation),
}

impl ValueOrBytecodeLocation {
    pub fn format(&self) -> String {
        match self {
            ValueOrBytecodeLocation::Value(i) => format!("{:016X}", i),
            ValueOrBytecodeLocation::BytecodeLocation(i) => format!("@{:>15}", format!("{}", i)),
        }
    }
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
pub enum InstructionTraceLocation {
    Bytecode {
        pc: BytecodeLocation,
        opcode: Opcode,
    },
    ParsedInstruction(Instruction<BytecodeRelativeOffset>),
    Event(String),
}

impl InstructionTraceLocation {
    pub fn format(&self) -> String {
        match self {
            InstructionTraceLocation::Bytecode { pc, opcode } => format!("{} {}", pc, opcode),
            InstructionTraceLocation::ParsedInstruction(i) => {
                format!(" - {:?}", i.map_labels(|x| x.0))
            }
            InstructionTraceLocation::Event(s) => format!(" E {}", s),
        }
    }
}

impl InstructionTraceLocation {
    pub fn is_bytecode(&self) -> bool {
        match self {
            InstructionTraceLocation::Bytecode { .. } => true,
            InstructionTraceLocation::ParsedInstruction(_) => false,
            InstructionTraceLocation::Event(_) => false,
        }
    }

    pub fn is_parsed_instruction(&self) -> bool {
        !self.is_bytecode()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InstructionTraceEntry {
    pub location: InstructionTraceLocation,
    pub accu: ValueOrBytecodeLocation,
    pub env: u64,
    pub extra_args: u64,
    pub sp: u64,
    pub trap_sp: u64,
    pub stack_size: usize,
    pub top_of_stack: Vec<ValueOrBytecodeLocation>,
}

impl InstructionTraceEntry {
    pub fn format(&self) -> String {
        format!(
            "{:<50} ACCU={} ENV={:016X} E_A={:<4} SP={:016X} TSP={:016X} SS={:<4} TOS={}",
            self.location.format(),
            self.accu.format(),
            self.env,
            self.extra_args,
            self.sp,
            self.trap_sp,
            self.stack_size,
            self.get_stack_display()
        )
    }

    pub fn print(&self) {
        println!("{}", self.format());
    }

    pub fn print_colored(&self) {
        println!(
            "{:<50} {}={} {}={:016X} {}={:<4} {}={:016X} {}={:016X} {}={:<4} {}={}",
            self.location.format(),
            "ACCU".bold(),
            self.accu.format(),
            "ENV".bold(),
            self.env,
            "E_A".bold(),
            self.extra_args,
            "SP".bold(),
            self.sp,
            "TSP".bold(),
            self.trap_sp,
            "SS".bold(),
            self.stack_size,
            "TOS".bold(),
            self.get_stack_display()
        );
    }

    fn get_stack_display(&self) -> String {
        let mut stack_display = String::new();
        let mut first = true;
        for v in self.top_of_stack.iter() {
            if first {
                first = false;
            } else {
                stack_display.push_str(", ");
            }
            stack_display.push_str(&v.format());
        }

        if self.top_of_stack.len() < self.stack_size {
            stack_display.push_str(", ...");
        }

        stack_display
    }
}

pub fn compare_instruction_traces(
    expected: &InstructionTraceEntry,
    actual: &InstructionTraceEntry,
) {
    println!("{}", expected.format().yellow().bold());
    if expected.location != actual.location {
        print!(
            "{} ",
            format!("{:<50}", actual.location.format()).red().bold()
        );
    } else {
        print!("{:<50} ", actual.location.format());
    }

    if expected.accu != actual.accu {
        print!("{} ", format!("ACCU={}", actual.accu.format()).red().bold());
    } else {
        print!("ACCU={} ", actual.accu.format());
    }

    if expected.env != actual.env {
        print!("{} ", format!("ENV={:016X}", actual.env).red().bold());
    } else {
        print!("ENV={:016X} ", actual.env);
    }

    if expected.extra_args != actual.extra_args {
        print!("{} ", format!("E_A={:<4}", actual.extra_args).red().bold());
    } else {
        print!("E_A={:<4} ", actual.extra_args);
    }

    if expected.sp != actual.sp {
        print!("{} ", format!("SP={:016X}", actual.sp).red().bold());
    } else {
        print!("SP={:016X} ", actual.sp);
    }

    if expected.trap_sp != actual.trap_sp {
        print!("{} ", format!("TSP={:016X}", actual.trap_sp).red().bold());
    } else {
        print!("TSP={:016X} ", actual.trap_sp);
    }

    if expected.stack_size != actual.stack_size {
        print!("{} ", format!("SS={:<4}", actual.stack_size).red().bold());
    } else {
        print!("SS={:<4} ", actual.stack_size);
    }

    print!("TOS=");
    let mut first = true;
    for (idx, v) in actual.top_of_stack.iter().enumerate() {
        if first {
            first = false;
        } else {
            print!(", ");
        }

        if expected.top_of_stack.get(idx) != Some(v) {
            print!("{}", v.format().red().bold());
        } else {
            print!("{}", v.format());
        }
    }

    println!();
}
