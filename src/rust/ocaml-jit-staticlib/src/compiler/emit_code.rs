use std::{
    collections::HashMap,
    convert::TryInto,
    ffi::{c_void, CStr},
};

use dynasmrt::{
    dynasm, x64::Assembler, AssemblyOffset, DynamicLabel, DynasmApi, DynasmLabelApi,
    ExecutableBuffer,
};

use ocaml_jit_shared::{
    ArithOp, BytecodeRelativeOffset, Closure, ClosureIterator, Comp, FoundClosure, Instruction,
    InstructionIterator,
};

use crate::{
    caml::{
        domain_state,
        domain_state::get_extern_sp_addr,
        mlvalues,
        mlvalues::{BlockValue, LongValue, Tag, Value},
    },
    compiler::{saved_data::LongjmpHandler, LongjmpEntryPoint},
};

use super::{c_primitives::*, rust_primitives::*, saved_data::EntryPoint};

pub const DEFAULT_HOT_CLOSURE_THRESHOLD: Option<usize> = Some(10);

#[derive(Debug, Copy, Clone)]
pub struct CompilerOptions {
    pub print_traces: bool,
    pub hot_closure_threshold: Option<usize>,
}

struct CompilerContext {
    ops: Assembler,
    labels: Vec<Option<DynamicLabel>>,
    compiler_options: CompilerOptions,
    section_number: usize,
    current_instruction_offset: BytecodeRelativeOffset,
    closures: HashMap<usize, ClosureData>,
}

struct ClosureData {
    label: DynamicLabel,
    arity: usize,
    bytecode_addr: usize,
}

pub struct CompilerResults {
    pub buffer: ExecutableBuffer,
    pub entrypoint: EntryPoint,
    pub first_instruction: *const c_void,
    pub instructions: Option<Vec<Instruction<BytecodeRelativeOffset>>>,
}

#[repr(C)]
#[derive(Debug)]
pub struct ClosureMetadataTableEntry {
    // Positive integer = execution count
    // -1 = restart, don't optimise
    // -2 = has been optimised
    // -3 = error while optimising, ignore this closure
    pub execution_count_status: i64,
    pub compiled_location: u64,
    pub section: u32,
    pub bytecode_offset: u32,
    pub required_extra_args: u64,
}

pub fn compile_instructions(
    section_number: usize,
    code: &[i32],
    compiler_options: CompilerOptions,
) -> CompilerResults {
    let mut ops = Assembler::new().unwrap();

    let labels = vec![None; code.len()];

    let mut instrs = if compiler_options.print_traces {
        Some(Vec::new())
    } else {
        None
    };

    let closures = scan_closures(&mut ops, code);

    let mut cc = CompilerContext {
        ops,
        labels,
        compiler_options,
        section_number,
        current_instruction_offset: BytecodeRelativeOffset(0),
        closures,
    };

    let (entrypoint_offset, first_instr_offset) = cc.emit_entrypoint();
    let code_base = code.as_ptr();

    for (offset, instruction) in InstructionIterator::new(code.iter().copied()).enumerate() {
        let instruction = instruction.unwrap();

        cc.emit_instruction(&instruction, offset, code_base);

        match &mut instrs {
            Some(v) => v.push(instruction),
            None => {}
        }
    }

    cc.emit_shared_code();
    cc.emit_closure_table();

    let ops = cc.ops;
    let buf = ops.finalize().unwrap();

    let entrypoint: EntryPoint = unsafe { std::mem::transmute(buf.ptr(entrypoint_offset)) };
    let first_instruction = buf.ptr(first_instr_offset) as *const c_void;

    CompilerResults {
        buffer: buf,
        entrypoint,
        first_instruction,
        instructions: instrs,
    }
}

fn scan_closures(ops: &mut Assembler, code: &[i32]) -> HashMap<usize, ClosureData> {
    let mut closures = HashMap::new();

    let mut add_closure = |func: Closure| {
        let old = closures.insert(
            func.code_offset,
            ClosureData {
                label: ops.new_dynamic_label(),
                bytecode_addr: func.code_offset,
                arity: func.arity,
            },
        );
        assert!(old.is_none());
    };

    for closure in ClosureIterator::new(code) {
        match closure {
            FoundClosure::Normal { func, .. } => {
                add_closure(func);
            }
            FoundClosure::Rec { funcs, .. } => {
                for func in funcs {
                    add_closure(func);
                }
            }
        }
    }

    closures
}

// Define aliases for the abstract machine registers
macro_rules! oc_dynasm {
    ($ops:expr; $($t:tt)*) => {
        let ops = &mut $ops;
        dynasm!(ops
            ; .arch x64
            ; .alias r_env, r12
            ; .alias r_accu, r13
            ; .alias r_extra_args, r14
            ; .alias r_sp, r15
            ; $($t)*
        )
    }
}

/* Alias for pushing a return address (used in PUSH_RETADDR and APPLY1-3)
 * Semantics:
 *
 * sp[offset] = (return label)
 * sp[offset + 1] = env
 * sp[offset + 2] = Val_long(extra_args)
 *
 * TRASHES: rax
 */
macro_rules! oc_pushretaddr {
    ($ops:expr, $offset:literal, $($retaddr:tt)*) => {
        oc_dynasm!($ops
            // Compute Val_long(extra_args) in rax
            ; mov rax, r_extra_args
            ; shl rax, 1
            ; inc rax
            // Make space and set stack
            ; mov [r_sp + $offset], $($retaddr)*
            ; mov [r_sp + 8 + $offset], r_env
            ; mov [r_sp + 16 + $offset], rax
       )
    }
}

/*
 * Callbacks are very strange and slightly annoying
 *
 * The base callback code is
 * opcode_t caml_callback_code[] = { ACC, 0, APPLY, 0, POP, 1, STOP };
 * in C.
 *
 * When a callback runs it sets up the stack as follows:
 *
 * Caml_state->extern_sp -= narg + 4;
 * for (i = 0; i < narg; i++) Caml_state->extern_sp[i] = args[i]; /* arguments */
 * Caml_state->extern_sp[narg] = (value)(caml_callback_code + 4); /* return address */
 * Caml_state->extern_sp[narg + 1] = Val_unit;    /* environment */
 * Caml_state->extern_sp[narg + 2] = Val_long(0); /* extra args */
 * Caml_state->extern_sp[narg + 3] = closure;
 * Init_callback();
 * caml_callback_code[1] = narg + 3;
 * caml_callback_code[3] = narg;
 *
 * - i.e. the return address is bytecode-relative and the argument to ACC and apply is changed
 * This of course won't do for the JIT - but we can do something with similar-ish semantics
 */
pub fn emit_callback_entrypoint(
    section_number: usize,
    compiler_options: CompilerOptions,
    code: &[i32],
) -> (ExecutableBuffer, EntryPoint, *const c_void) {
    // We don't actually use the labels, but we need it for a context
    let labels = vec![None; 0];
    let ops = Assembler::new().unwrap();
    let mut cc = CompilerContext {
        ops,
        labels,
        compiler_options,
        section_number,
        closures: HashMap::new(),
        current_instruction_offset: BytecodeRelativeOffset(0),
    };

    let (entrypoint_offset, first_instr_offset) = cc.emit_entrypoint();
    let code_base = code.as_ptr();

    oc_dynasm!(&mut cc.ops
        // Get narg from caml_callback_code[3] and store in rbx
        ; mov rsi, QWORD ((code_base as usize) + (3 * 4)) as i64
        ; mov ebx, [rsi]
        // Fix the location of the return address on the stack
        ; lea rdi, [>return_after_callback]
        ; mov [r_sp + rbx * 8], rdi
    );

    // Emit a trace for the ACC - rbx won't be trashed
    if compiler_options.print_traces {
        cc.emit_bytecode_trace(code_base, &BytecodeRelativeOffset(0));
    }
    // Actually perform the acc - it's nargs + 3
    oc_dynasm!(&mut cc.ops
        ; mov rax, rbx
        ; add rax, 3
        ; mov r_accu, [r_sp + rax * 8]
    );

    // Emit a trace for the apply
    if compiler_options.print_traces {
        cc.emit_bytecode_trace(code_base, &BytecodeRelativeOffset(2));
    }
    // Perform the apply - it's nargs - 1 for the new extra_args
    oc_dynasm!(&mut cc.ops
        ; mov r_extra_args, rbx
        ; dec r_extra_args
    );
    cc.perform_apply();

    // Return - POP 1, STOP
    oc_dynasm!(&mut cc.ops
        ; return_after_callback:
    );
    // Emit a trace for the POP
    if compiler_options.print_traces {
        cc.emit_bytecode_trace(code_base, &BytecodeRelativeOffset(4));
    }
    oc_dynasm!(&mut cc.ops
        ; add r_sp, 8
    );
    // Emit a trace for the STOP
    if compiler_options.print_traces {
        cc.emit_bytecode_trace(code_base, &BytecodeRelativeOffset(6));
    }
    // Emit the actual stop
    oc_dynasm!(&mut cc.ops
        ; mov rax, QWORD jit_support_stop as i64
        ; mov rdi, [rsp]
        ; mov rsi, r_sp
        ; call rax
        // Set the return value
        ; mov rax, r_accu
    );
    cc.emit_return();
    cc.emit_shared_code();

    let ops = cc.ops;
    let buf = ops.finalize().unwrap();
    let entrypoint: EntryPoint = unsafe { std::mem::transmute(buf.ptr(entrypoint_offset)) };
    let first_instr = buf.ptr(first_instr_offset) as *const c_void;

    (buf, entrypoint, first_instr)
}

// Enums to avoid magic constants
enum NextInstruction {
    RestartOrAfter,
    GoToNext,
    UseRSI,
}

fn caml_i32_of_int(orig: i64) -> i32 {
    Value::from(LongValue::from_i64(orig)).0 as i32
}

pub fn emit_longjmp_entrypoint() -> LongjmpHandler {
    /* For handling exceptions raised by C primitives the existing runtime uses longjmp
     * The code for the interpreter just jumps to the raise code.
     *
     * To replicate this with the jit, we push a function that sets up the C stack in
     * the same way as emit_entrypoint and also does the things that happens when we call
     * raise - see jit_support_main_wrapper in the C primitives for how this gets used
     */
    let mut ops = Assembler::new().unwrap();
    let start_offset = ops.offset();
    oc_dynasm!(ops
        // Push callee-save registers I use
        ; push r_accu
        ; push r_env
        ; push r_extra_args
        ; push r_sp
        // Push the pointer to the initial state struct
        ; push rdi
        // Store the initial accu
        ; mov r_accu, rsi
        // Get the trapsp address
        ; mov rsi, QWORD domain_state::get_trap_sp_addr() as usize as i64
        // Set the sp from it
        ; mov r_sp, [rsi]
        // Set the new trap sp to the next one in the link
        ; mov rax, [r_sp + 8]
        ; mov [rsi], rax
        // Restore the env
        ; mov r_env, [r_sp + 16]
        // Restore the extra args - un-Val_long it
        ; mov r_extra_args, [r_sp + 24]
        ; shr r_extra_args, 1
        // Save location to jump, increment sp and go to it
        ; mov rax, [r_sp]
        ; add r_sp, 32
        ; jmp rax
    );

    let buf = ops.finalize().unwrap();
    let entrypoint: LongjmpEntryPoint = unsafe { std::mem::transmute(buf.ptr(start_offset)) };
    LongjmpHandler {
        compiled_code: buf,
        entrypoint,
    }
}

impl CompilerContext {
    fn get_label(&mut self, offset: &BytecodeRelativeOffset) -> DynamicLabel {
        let label_ref = &mut self.labels[offset.0];
        match label_ref {
            Some(l) => *l,
            None => {
                let label = self.ops.new_dynamic_label();
                *label_ref = Some(label);
                label
            }
        }
    }

    fn emit_entrypoint(&mut self) -> (AssemblyOffset, AssemblyOffset) {
        let offset = self.ops.offset();
        oc_dynasm!(self.ops
            // Push callee-save registers I use
            ; push r_accu
            ; push r_env
            ; push r_extra_args
            ; push r_sp
            // Push the pointer to the initial state struct
            ; push rdi
            // We're now aligned for the C calling convention
            // Set up initial register values
            ; mov r_accu, caml_i32_of_int(0)
            ; mov r_env, QWORD BlockValue::atom(Tag(0)).0
            ; mov r_extra_args, 0
            // The first field in the initial state struct is the initial sp value to use
            // That's the thing on the top of the stack
            ; mov rax, [rsp]
            ; mov r_sp, [rax]
        );

        let first_instr_offset = self.ops.offset();
        (offset, first_instr_offset)
    }

    fn emit_return(&mut self) {
        // Clean up what the initial code did and return to the caller
        oc_dynasm!(self.ops
            // Undo push of initial state pointer
            ; add rsp, 8
            // Undo original pushes
            ; pop r_sp
            ; pop r_extra_args
            ; pop r_env
            ; pop r_accu
            ; ret
        );
    }

    fn emit_bytecode_trace(
        &mut self,
        code_base: *const i32,
        bytecode_offset: &BytecodeRelativeOffset,
    ) {
        let bytecode_pointer =
            unsafe { code_base.offset((bytecode_offset.0 as usize).try_into().unwrap()) };
        oc_dynasm!(self.ops
            ; mov rdi, QWORD bytecode_pointer as i64
            ; mov rsi, r_accu
            ; mov rdx, r_env
            ; mov rcx, r_extra_args
            ; mov r8, r_sp
            ; mov rax, QWORD bytecode_trace as i64
            ; call rax
        );
    }

    fn emit_instruction(
        &mut self,
        instruction: &Instruction<BytecodeRelativeOffset>,
        offset: usize,
        code_base: *const i32,
    ) -> Option<()> {
        if let Instruction::LabelDef(bytecode_offset) = instruction {
            self.current_instruction_offset = *bytecode_offset;
            let label = self.get_label(bytecode_offset);
            oc_dynasm!(self.ops
                ; =>label
                ; instr:
            );

            if self.compiler_options.print_traces {
                self.emit_bytecode_trace(code_base, bytecode_offset);
            }
        }

        if self.compiler_options.print_traces {
            oc_dynasm!(self.ops
                ; mov rdi, QWORD offset as i64
                ; mov rsi, r_accu
                ; mov rdx, r_env
                ; mov rcx, r_extra_args
                ; mov r8, r_sp
                ; mov r9, self.section_number as i32
                ; mov rax, QWORD instruction_trace as i64
                ; call rax
            );
        }

        match instruction {
            Instruction::LabelDef(_) => {}
            Instruction::Acc(n) => {
                // accu = sp[*pc++]
                let offset = (n * 8) as i32;
                oc_dynasm!(self.ops
                    ; mov r_accu, [r_sp + offset]
                );
            }
            Instruction::EnvAcc(n) => {
                // accu = Field(env, n)
                // For now let a function do the work of getting the field
                let field_no = *n as i64;
                oc_dynasm!(self.ops
                    ; mov r_accu, [r_env + (field_no * 8) as _]
                );
            }
            Instruction::Push => {
                // *--sp = accu
                oc_dynasm!(self.ops
                    ; sub r_sp, 8
                    ; mov [r_sp], r_accu
                );
            }
            Instruction::Pop(n) => {
                // sp += n
                let offset = (n * 8) as i32;
                oc_dynasm!(self.ops
                    ; add r_sp, offset
                );
            }
            Instruction::Assign(n) => {
                // sp[n] = accu;
                // accu = Val_unit;
                let offset = (n * 8) as i32;
                oc_dynasm!(self.ops
                    ; mov [r_sp + offset], r_accu
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                );
            }
            // There are two ways to call something in OCaml bytecode
            // (eval args), APPLY{1,2,3}
            // PushRetAddr, (eval args), APPLY n
            // We support both
            Instruction::PushRetAddr(offset) => {
                // sp[0] = (return label)
                // sp[1] = env
                // sp[2] = Val_long(extra_args)
                let return_label = self.get_label(offset);
                oc_dynasm!(self.ops
                    ; sub r_sp, 24
                    ; lea rcx, [=>return_label]
                );
                oc_pushretaddr!(self.ops, 0, rcx);
            }
            Instruction::Apply(0) => panic!("Apply(0) found!"),
            Instruction::Apply1 => {
                oc_dynasm!(self.ops
                    // Save the first argument, drop the sp and restore it
                    ; mov rax, [r_sp]
                    ; sub r_sp, 24
                    ; mov [r_sp], rax
                    ; lea rcx, [>instr]   // Save return location (next instruction)
                );
                // Push the return frame (retaddr, num_args, env)
                oc_pushretaddr!(self.ops, 8, rcx);
                oc_dynasm!(self.ops
                    // Set the env and extra_args to the new appropriate values
                    ; mov r_extra_args, 0
                );
                self.perform_apply();
            }
            Instruction::Apply2 => {
                // Like for Apply(1), but saving two args
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; mov rcx, [r_sp + 8]
                    ; sub r_sp, 24
                    ; mov [r_sp], rax
                    ; mov [r_sp + 8], rcx
                    ; lea rcx, [>instr]
                );
                oc_pushretaddr!(self.ops, 16, rcx);
                oc_dynasm!(self.ops
                    ; mov r_extra_args, 1
                );

                self.perform_apply();
            }
            Instruction::Apply3 => {
                // Like one, but saving two args
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; mov rcx, [r_sp + 8]
                    ; mov rdx, [r_sp + 16]
                    ; sub r_sp, 24
                    ; mov [r_sp], rax
                    ; mov [r_sp + 8], rcx
                    ; mov [r_sp + 16], rdx
                    ; lea rcx, [>instr]   // Save return location
                );
                oc_pushretaddr!(self.ops, 24, rcx);
                oc_dynasm!(self.ops
                    ; mov r_extra_args, 2
                );

                self.perform_apply();
            }
            Instruction::Apply(n) => {
                // In any other cases the retaddr is already pushed
                // So just set extra args, pc and jump to the closure's pc
                let new_extra_args = (*n - 1) as i32;
                oc_dynasm!(self.ops
                    ; mov r_extra_args, new_extra_args
                );

                self.perform_apply();
            }
            Instruction::ApplyTerm(nargs, slotsize) => {
                let nargs = *nargs as i32;
                let slotsize = *slotsize as i32;
                // for now we're calling into C for the offset
                oc_dynasm!(self.ops
                    ; mov rdi, nargs
                    ; mov rsi, slotsize
                    ; mov rdx, r_sp
                    ; add r_extra_args, nargs - 1
                    ; mov rax, QWORD jit_support_appterm_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                );
                self.perform_apply();
            }
            Instruction::Return(to_pop) => {
                oc_dynasm!(self.ops
                    ; add r_sp, (*to_pop as i32) * 8
                    ; test r_extra_args, r_extra_args
                    ; jnz >tailcall
                    ; mov rax, [r_sp]
                    ; mov r_env, [r_sp + 8]
                    ; mov r_extra_args, [r_sp + 16]
                    ; sar r_extra_args, BYTE 1
                    ; add r_sp, 24
                    ; jmp rax
                    ; tailcall:
                    ; dec r_extra_args
                );
                self.perform_apply();
            }
            Instruction::Restart | Instruction::Grab(_) => {
                // Do nothing, we don't use them in the way the interpreter does
            }
            Instruction::Closure(codeval, nargs) => {
                let nargs = *nargs as i32;
                let closure = self.closures.get(&codeval.0).unwrap();
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov rsi, nargs
                    ; lea rdx, [=>closure.label]
                    ; mov rax, QWORD jit_support_closure as i64
                    ; call rax
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                );
            }
            Instruction::ClosureRec(funcs, nvars) => {
                // Set up for a call
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov rsi, *nvars as i32
                );

                // Push all of the functions also onto the stack and put a pointer in the
                // argument position
                for offset in funcs.iter().rev() {
                    let closure = self.closures.get(&offset.0).unwrap();
                    oc_dynasm!(self.ops
                        ; lea rax, [=>closure.label]
                        ; push rax
                    );
                }
                oc_dynasm!(self.ops
                    ; mov rdx, rsp
                    ; mov ecx, funcs.len() as i32
                );

                let unaligned = funcs.len() % 2 == 1;
                if unaligned {
                    oc_dynasm!(self.ops
                        ; sub rsp, 8
                    );
                }

                let to_pop = (funcs.len() + if unaligned { 1 } else { 0 }) as i32;
                oc_dynasm!(self.ops
                    // Call c support function
                    ; mov rax, QWORD jit_support_closure_rec as i64
                    ; call rax
                    // Pop off functions from stack + alignment
                    ; add rsp, to_pop * 8
                    // Pop the actual registers
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                );
            }
            Instruction::OffsetClosure(n) => {
                oc_dynasm!(self.ops
                    ; mov r_accu, r_env
                    ; add r_accu, (*n as i32) * 8
                );
            }
            Instruction::GetGlobal(field_no) => {
                oc_dynasm!(self.ops
                    // TODO - look into if I can optimise this if I know it fit's in 32 bits
                    // Or if ASLR messes things up somehow, store the high bits in a spare reg
                    // and use it to index all of my accesses? Likewise with function calls.
                    ; mov rax, QWORD get_global_data_addr()
                    ; mov rdi, [rax]
                    ; mov r_accu, [rdi + (*field_no * 8) as _]
                );
            }
            Instruction::SetGlobal(field_no) => {
                oc_dynasm!(self.ops
                    ; mov rax, QWORD get_global_data_addr()
                    ; mov rdi, [rax]
                    ; add rdi, (*field_no * 8) as _
                    ; mov rsi, r_accu
                    ; mov rax, QWORD caml_modify as i64
                    ; call rax
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                );
            }
            Instruction::Const(i) => {
                oc_dynasm!(self.ops
                    ; mov eax, caml_i32_of_int(*i as i64)
                    ; movsxd r_accu, eax
                );
            }
            Instruction::MakeBlock(0, tag) => {
                oc_dynasm!(self.ops
                    ; mov r_accu, QWORD BlockValue::atom(Tag(*tag)).0
                );
            }
            Instruction::MakeBlock(wosize, tag) => {
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov esi, *wosize as i32
                    ; mov edx, *tag as i32
                    ; mov rax, QWORD jit_support_make_block as i64
                    ; call rax
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                );
            }
            Instruction::MakeFloatBlock(size) => {
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov esi, *size as i32
                    ; mov rax, QWORD jit_support_make_float_block as i64
                    ; call rax
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                );
            }
            Instruction::GetField(field_no) => {
                oc_dynasm!(self.ops
                    ; mov r_accu, [r_accu + (*field_no * 8) as _]
                );
            }
            Instruction::SetField(field_no) => {
                oc_dynasm!(self.ops
                    ; mov rdi, r_accu
                    ; add rdi, (*field_no * 8) as i32
                    ; mov rsi, [r_sp]
                    ; mov rax, QWORD caml_modify as i64
                    ; call rax
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                    ; add r_sp, 8
                );
            }
            Instruction::GetFloatField(i) => {
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov esi, *i as i32
                    ; mov rax, QWORD jit_support_get_float_field as i64
                    ; call rax
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                    ; mov r_accu, rax
                );
            }
            Instruction::SetFloatField(i) => {
                oc_dynasm!(self.ops
                    ; mov rdi, r_accu
                    ; mov esi, *i as i32
                    ; mov rdx, [r_sp]
                    ; mov rax, QWORD jit_support_set_float_field as i64
                    ; call rax
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                    ; add r_sp, 8
                );
            }
            Instruction::VecTLength => {
                oc_dynasm!(self.ops
                    ; mov rdi, r_accu
                    ; mov rax, QWORD jit_support_vect_length as i64
                    ; call rax
                    ; mov r_accu, rax
                );
            }
            Instruction::GetVecTItem => {
                oc_dynasm!(self.ops
                    ; mov esi, [r_sp]
                    ; shr esi, 1
                    ; mov r_accu, [r_accu + rsi * 8]
                    ; add r_sp, 8
                );
            }
            Instruction::SetVecTItem => {
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; shr rax, 1
                    ; lea rdi, [r_accu + rax * 8]
                    ; mov rsi, [r_sp + 8]
                    ; mov rax, QWORD caml_modify as i64
                    ; call rax
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                    ; add r_sp, 2*8
                );
            }
            Instruction::Branch(loc) => {
                let label = self.get_label(loc);
                oc_dynasm!(self.ops
                    ; jmp =>label
                );
            }
            Instruction::BranchIf(loc) => {
                let label = self.get_label(loc);
                oc_dynasm!(self.ops
                    ; cmp r_accu, BYTE 1 // Which is Val_false
                    ; je >next
                    ; jmp =>label
                    ; next:
                );
            }
            Instruction::BranchIfNot(loc) => {
                let label = self.get_label(loc);
                oc_dynasm!(self.ops
                    ; cmp r_accu, BYTE 1 // Which is Val_false
                    ; jne >next
                    ; jmp =>label
                    ; next:
                );
            }
            Instruction::Switch(ints, blocks) => {
                // Really inefficient for now
                // TODO investigate how best to emit a jump table
                for (i, offset) in ints.iter().enumerate() {
                    let label = self.get_label(offset);
                    oc_dynasm!(self.ops
                        ; mov eax, caml_i32_of_int(i as i64)
                        ; movsx rax, eax
                        ; cmp r_accu, rax
                        ; je =>label
                    );
                }

                // Ok it's not an int
                oc_dynasm!(self.ops
                    ; mov rax, [r_accu - 8]
                );

                for (tag, offset) in blocks.iter().enumerate() {
                    let label = self.get_label(offset);
                    oc_dynasm!(self.ops
                        ; cmp al, tag as i8
                        ; je =>label
                    );
                }

                self.emit_fatal_error(b"Switch - should be unreachable!\0")
            }
            Instruction::PushTrap(loc) => {
                let label = self.get_label(loc);
                let trap_sp = domain_state::get_trap_sp_addr();
                oc_dynasm!(self.ops
                    // Get the trapsp address
                    ; mov rsi, QWORD (trap_sp as usize) as i64
                    // Push the trap frame
                    ; sub r_sp, 32
                    // Push pc to go to
                    ; lea rcx, [=>label]
                    ; mov [r_sp], rcx
                    // Push current trapsp
                    ; mov rax, [rsi]
                    ; mov [r_sp + 8], rax
                    // Push current env
                    ; mov [r_sp + 16], r_env
                    // Push Val_long(extra_args)
                    ; mov rax, r_extra_args
                    ; shl rax, 1
                    ; inc rax
                    ; mov [r_sp + 24], rax
                    // Set the trapsp to current sp
                    ; mov [rsi], r_sp
                );
            }
            Instruction::PopTrap => {
                let trap_sp = domain_state::get_trap_sp_addr();
                self.emit_check_signals(NextInstruction::RestartOrAfter);
                oc_dynasm!(self.ops
                    ; after:
                    // Get the trapsp address
                    ; mov rax, QWORD (trap_sp as usize) as i64
                    ; mov rcx, [r_sp + 8]
                    ; mov [rax], rcx
                    ; add r_sp, 32
                );
            }
            Instruction::Raise(_kind) => {
                let trap_sp = domain_state::get_trap_sp_addr();
                // TODO backtraces, checking if the trapsp is above initial sp offest
                oc_dynasm!(self.ops
                    // Check if we've gone too high in the stack
                    ; mov rdi, [rsp]  // Initial state pointer
                    ; mov rsi, r_sp   // Current sp
                    ; mov rax, QWORD jit_support_raise_check as i64
                    ; call rax
                    ; cmp rax, 0
                    ; jne >return_exception_result

                    // Ok, not too high, can do the link stuff
                    // Get the current trap sp
                    ; mov rsi, QWORD (trap_sp as usize) as i64
                    // Set the sp from it
                    ; mov r_sp, [rsi]
                    // Set the new trap sp to the next one in the link
                    ; mov rax, [r_sp + 8]
                    ; mov [rsi], rax
                    // Restore the env
                    ; mov r_env, [r_sp + 16]
                    // Restore the extra args - un-Val_long it
                    ; mov r_extra_args, [r_sp + 24]
                    ; shr r_extra_args, 1
                    // Save location to jump, increment sp and go to it
                    ; mov rax, [r_sp]
                    ; add r_sp, 32
                    ; jmp rax

                    // Otherwise
                    ; return_exception_result:
                    ; mov rax, r_accu
                    ; or rax, 2
                );
                self.emit_return();
            }
            Instruction::CCall1(primno) => {
                // TODO - possible optimisation, could load the static address
                // if it's currently in the table
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
            }
            Instruction::CCall2(primno) => {
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp + 2 * 8]
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
                oc_dynasm!(self.ops
                    ; add r_sp, 8
                );
            }
            Instruction::CCall3(primno) => {
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp + 2 * 8]
                    ; mov rdx, [r_sp + 3 * 8]
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
                oc_dynasm!(self.ops
                    ; add r_sp, 16
                );
            }
            Instruction::CCall4(primno) => {
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp + 2 * 8]
                    ; mov rdx, [r_sp + 3 * 8]
                    ; mov rcx, [r_sp + 4 * 8]
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
                oc_dynasm!(self.ops
                    ; add r_sp, 24
                );
            }
            Instruction::CCall5(primno) => {
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp + 2 * 8]
                    ; mov rdx, [r_sp + 3 * 8]
                    ; mov rcx, [r_sp + 4 * 8]
                    ; mov r8, [r_sp + 5 * 8]
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
                oc_dynasm!(self.ops
                    ; add r_sp, BYTE 32
                );
            }
            Instruction::CCallN(nargs, primno) => {
                let nargs = *nargs as i32;
                oc_dynasm!(self.ops
                    ; sub r_sp, BYTE 8
                    ; mov [r_sp], r_accu
                );
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; lea rdi, [r_sp + 2 * 8]
                    ; mov rsi, nargs
                    ; call rax
                    ; mov r_accu, rax
                );
                self.restore_after_c_call();
                oc_dynasm!(self.ops
                    ; add r_sp, 8 * nargs
                );
            }
            Instruction::NegInt => {
                oc_dynasm!(self.ops
                    ; mov rax, 2
                    ; sub rax, r_accu
                    ; mov r_accu, rax
                );
            }
            Instruction::ArithInt(ArithOp::Mul) => {
                oc_dynasm!(self.ops
                    // Convert from ocaml longs to actual longs, multiply, convert back
                    ; mov rax, [r_sp]
                    ; sar rax, 1
                    ; mov rdx, rax
                    ; mov rax, r_accu
                    ; sar rax, 1
                    ; imul rax, rdx
                    ; add rax, rax
                    ; add rax, 1
                    ; mov r_accu, rax
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Add) => {
                oc_dynasm!(self.ops
                    ; add r_accu, [r_sp]
                    ; dec r_accu
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Sub) => {
                oc_dynasm!(self.ops
                    ; sub r_accu, [r_sp]
                    ; inc r_accu
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Div) => {
                oc_dynasm!(self.ops
                    // Convert from ocaml longs to actual longs, divide, convert back
                    ; mov rax, [r_sp]
                    ; sar rax, 1
                    ; cmp rax, 0
                    ; je >div0
                    ; mov rdx, rax
                    ; mov rcx, rdx
                    ; mov rax, r_accu
                    ; sar rax, 1
                    ; cqo
                    ; idiv rcx
                    ; add rax, rax
                    ; add rax, 1
                    ; mov r_accu, rax
                    ; add r_sp, BYTE 8
                    ; jmp >next
                    // Raise divide 0
                    ; div0:
                );
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rax, QWORD caml_raise_zero_divide as i64
                    ; call rax
                    ; next:
                );
            }
            Instruction::ArithInt(ArithOp::Mod) => {
                // As div, but using rdx which has the remainder in it
                oc_dynasm!(self.ops
                    // Convert from ocaml longs to actual longs, mod, convert back
                    ; mov rax, [r_sp]
                    ; sar rax, 1
                    ; cmp rax, 0
                    ; je >div0
                    ; mov rdx, rax
                    ; mov rcx, rdx
                    ; mov rax, r_accu
                    ; sar rax, 1
                    ; cqo
                    ; idiv rcx
                    ; add rdx, rdx
                    ; add rdx, 1
                    ; mov r_accu, rdx
                    ; add r_sp, BYTE 8
                    ; jmp >next
                    // Raise divide 0
                    ; div0:
                );
                self.setup_for_c_call();
                oc_dynasm!(self.ops
                    ; mov rax, QWORD caml_raise_zero_divide as i64
                    ; call rax
                    ; next:
                );
            }
            Instruction::ArithInt(ArithOp::Or) => {
                oc_dynasm!(self.ops
                    ; or r_accu, [r_sp]
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::And) => {
                oc_dynasm!(self.ops
                    ; and r_accu, [r_sp]
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Xor) => {
                oc_dynasm!(self.ops
                    ; xor r_accu, [r_sp]
                    ; or r_accu, BYTE 1
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Lsl) => {
                oc_dynasm!(self.ops
                    ; mov ecx, [r_sp]
                    ; shr ecx, BYTE 1
                    ; dec r_accu
                    ; shl r_accu, cl
                    ; inc r_accu
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Lsr) => {
                oc_dynasm!(self.ops
                    ; mov ecx, [r_sp]
                    ; shr ecx, BYTE 1
                    ; shr r_accu, cl
                    ; or r_accu, BYTE 1
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::ArithInt(ArithOp::Asr) => {
                oc_dynasm!(self.ops
                    ; mov ecx, [r_sp]
                    ; shr ecx, BYTE 1
                    ; sar r_accu, cl
                    ; or r_accu, BYTE 1
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::BoolNot => {
                oc_dynasm!(self.ops
                    ; xor r_accu, BYTE 2
                );
            }
            Instruction::IntCmp(cmp) => {
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; mov rcx, r_accu
                    ; add r_sp, BYTE 8
                    ; mov rdi, 3 // Val_true
                    ; mov r_accu, 1 // Val_false
                    ; cmp rcx, rax
                );
                match cmp {
                    Comp::Eq => {
                        oc_dynasm!(self.ops
                            ; cmove r_accu, rdi
                        );
                    }
                    Comp::Ne => {
                        oc_dynasm!(self.ops
                            ; cmovne r_accu, rdi
                        );
                    }
                    Comp::Lt => {
                        oc_dynasm!(self.ops
                            ; cmovl r_accu, rdi
                        );
                    }
                    Comp::Le => {
                        oc_dynasm!(self.ops
                            ; cmovle r_accu, rdi
                        );
                    }
                    Comp::Gt => {
                        oc_dynasm!(self.ops
                            ; cmovg r_accu, rdi
                        );
                    }
                    Comp::Ge => {
                        oc_dynasm!(self.ops
                            ; cmovge r_accu, rdi
                        );
                    }
                    Comp::ULt => {
                        oc_dynasm!(self.ops
                            ; cmovb r_accu, rdi
                        );
                    }
                    Comp::UGe => {
                        oc_dynasm!(self.ops
                            ; cmovae r_accu, rdi
                        );
                    }
                }
            }
            Instruction::BranchCmp(cmp, i, l) => {
                let label = self.get_label(l);
                oc_dynasm!(self.ops
                    ; mov eax, caml_i32_of_int(*i as i64)
                    ; movsxd rcx, eax
                    ; cmp rcx, r_accu
                );
                match cmp {
                    Comp::Eq => {
                        oc_dynasm!(self.ops
                            ; je =>label
                        );
                    }
                    Comp::Ne => {
                        oc_dynasm!(self.ops
                            ; jne =>label
                        );
                    }
                    Comp::Lt => {
                        oc_dynasm!(self.ops
                            ; jl =>label
                        );
                    }
                    Comp::Le => {
                        oc_dynasm!(self.ops
                            ; jle =>label
                        );
                    }
                    Comp::Gt => {
                        oc_dynasm!(self.ops
                            ; jg =>label
                        );
                    }
                    Comp::Ge => {
                        oc_dynasm!(self.ops
                            ; jge =>label
                        );
                    }
                    Comp::ULt => {
                        oc_dynasm!(self.ops
                            ; jb =>label
                        );
                    }
                    Comp::UGe => {
                        oc_dynasm!(self.ops
                            ; jae =>label
                        );
                    }
                }
            }
            Instruction::OffsetInt(n) => {
                oc_dynasm!(self.ops
                    ; mov ecx, *n as i32
                    ; shl ecx, BYTE 1
                    ; movsxd rax, ecx
                    ; add r_accu, rax
                );
            }
            Instruction::OffsetRef(n) => {
                oc_dynasm!(self.ops
                    ; mov ecx, *n as i32
                    ; shl ecx, BYTE 1
                    ; movsxd rax, ecx
                    ; add [r_accu], rax
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                );
            }
            Instruction::IsInt => {
                oc_dynasm!(self.ops
                    ; and r_accu, 1
                    ; shl r_accu, 1
                    ; add r_accu, 1
                );
            }
            Instruction::GetBytesChar => {
                oc_dynasm!(self.ops
                    ; mov rsi, [r_sp]
                    ; shr rsi, BYTE 1
                    ; movzx r_accu, BYTE [r_accu + rsi]
                    ; add r_accu, r_accu
                    ; inc r_accu
                    ; add r_sp, BYTE 8
                );
            }
            Instruction::SetBytesChar => {
                oc_dynasm!(self.ops
                    ; mov rsi, [r_sp]
                    ; shr rsi, BYTE 1
                    ; mov rax, [r_sp + 8]
                    ; shr rax, BYTE 1
                    ; mov BYTE [r_accu + rsi], al
                    ; add r_sp, BYTE 16
                    ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
                );
            }
            Instruction::Stop => {
                // Call the function so that the entrypoint and code that uses it is visually nearby
                // for easier changes
                oc_dynasm!(self.ops
                    ; mov rax, QWORD jit_support_stop as i64
                    ; mov rdi, [rsp]
                    ; mov rsi, r_sp
                    ; call rax
                    // Set the return value
                    ; mov rax, r_accu
                );
                self.emit_return();
            }
            Instruction::CheckSignals => {
                self.emit_check_signals(NextInstruction::GoToNext);
            }
            Instruction::GetMethod => {
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; mov rax, [rax]
                    ; shr r_accu, 1
                    ; mov r_accu, [rax + r_accu * 8]
                );
            }
            Instruction::SetupForPubMet(tag) => {
                oc_dynasm!(self.ops
                    // *--sp = accu
                    ; sub r_sp, 8
                    ; mov [r_sp], r_accu
                    // accu = Val_int(*pc);
                    ; mov rax, *tag
                    ; add rax, rax
                    ; inc rax
                    ; mov r_accu, rax
                );
            }
            Instruction::GetDynMet => {
                oc_dynasm!(self.ops
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp]
                    ; mov rax, QWORD jit_support_get_dyn_met as _
                    ; call rax
                    ; mov r_accu, rax
                );
            }
            // Unimplemented ops:
            Instruction::Break => {
                self.emit_fatal_error(b"Unimplemented: Break\0");
            }
            Instruction::Event => {
                // Events shouldn't ever be emitted in the current compiler version
                // but in any case it's a no-op
            }
        }

        Some(())
    }

    fn perform_apply(&mut self) {
        // This used to inline the stuff, but I'm changing it to jump to one apply function
        oc_dynasm!(self.ops
            ; jmp ->apply
        );
    }

    fn emit_check_signals(&mut self, next_instr: NextInstruction) {
        oc_dynasm!(self.ops
            ; mov rax, QWORD get_something_to_do_addr()
            ; mov rdi, [rax]
            ; test rdi, rdi
        );

        match next_instr {
            NextInstruction::RestartOrAfter => {
                oc_dynasm!(self.ops
                    ; jz >after            // go to next if we don't need to
                    ; lea rsi, [<instr]    // otherwise set up to restart current instr
                    ; jmp ->process_events
                );
            }
            NextInstruction::GoToNext => {
                oc_dynasm!(self.ops
                    ; jz >instr            // jump to next
                    ; lea rsi, [>instr]    // used in setup for event
                    ; jmp ->process_events
                );
            }
            NextInstruction::UseRSI => {
                oc_dynasm!(self.ops
                    ; jnz ->process_events
                    ; jmp rsi
                );
            }
        }
    }

    fn emit_shared_code(&mut self) {
        self.emit_apply_shared();
        self.emit_process_events_shared();
    }

    fn emit_apply_shared(&mut self) {
        // Check stacks, checks signals then jumps to the closure stored in the accu
        oc_dynasm!(self.ops
            ; ->apply:
            ; mov r_env, r_accu
            // Check stacks
            ; mov rdi, r_sp
            ; mov rax, QWORD jit_support_check_stacks as i64
            ; call rax
            ; mov r_sp, rax

            // Check if we're doing a restart
            ; mov rax, [r_accu]
            ; mov rsi, [rax]
            ; cmp rsi, -1
            ; je >bytecall             // If it's a restart, jump to call

            // Check for extra args (performs GRAB, but at call time)
            ; mov rsi, [rax + 0x18]    // load the required extra args from closure metadata
            ; cmp r_extra_args, rsi
            ; jl >make_restart_closure // make a closure instead if parital application
            ; sub r_extra_args, rsi    // otherwise subtract required from extra_args
        );

        // If we're enabling the JIT, do the counting/branching to compiler logic
        if let Some(threshold) = self.compiler_options.hot_closure_threshold {
            oc_dynasm!(self.ops
                ; check_opt:
                // Check if the closure's already optimised
                ; mov rsi, [rax]
                ; cmp rsi, -2
                ; je >optcall

                // Increment the counter
                ; inc rsi
                ; mov [rax], rsi

                // Check if we're now above the threshold
                ; cmp rsi, threshold as _
                ; jl >bytecall
            );
            self.emit_event(b"Hot closure detected, compiling\0");
            oc_dynasm!(self.ops
                // Branch to the optimised compiler
                ; mov rdi, [r_accu]
                ; mov rax, QWORD compile_closure_optimised as i64
                ; call rax
                // Go back and try to re-run the closure
                // If compilation failed, the status will reflect that
                // and we'll no longer try to re-run
                ; mov rax, [r_accu]
                ; jmp <check_opt
            );
        }

        oc_dynasm!(self.ops
            ; optcall:
            // for now, we don't actually optimise

            ; bytecall:
            // Check signals - then jump to the PC saved in the closure
            ; mov rsi, [rax + 8]
        );
        if self.compiler_options.print_traces {
            // Needed for the trace comparison code to be happy, but not needed when actually running
            oc_dynasm!(self.ops
                ; mov r_accu, mlvalues::LongValue::UNIT.0 as i32
            );
        }
        self.emit_check_signals(NextInstruction::UseRSI);

        // Code for making a new closure on partial application, replacing Grab
        oc_dynasm!(self.ops

            ; make_restart_closure:
            ; mov r_env, r_accu   // set accu as if we're jumping
            ; push r_extra_args
            ; push r_sp
            ; push r_env
            ; push r_accu
            ; mov rdi, rsp
            ; lea rsi, [->restart_closure]
            ; mov rax, QWORD jit_support_grab_closure as i64
            ; call rax
            ; pop r_accu
            ; pop r_env
            ; pop r_sp
            ; pop r_extra_args
            ; jmp rax
        );

        // Emit shared restart code
        oc_dynasm!(self.ops
            ; ->restart_code:
            ; push r_extra_args
            ; push r_sp
            ; push r_env
            ; push r_accu
            ; mov rdi, rsp
            ; mov rax, QWORD jit_support_restart as i64
            ; call rax
            ; pop r_accu
            ; pop r_env
            ; pop r_sp
            ; pop r_extra_args
            ; jmp ->apply
        );

        // Emit the restart closure (see emit_closure_table for format)
        oc_dynasm!(self.ops
            ; ->restart_closure:
            ; .qword -1                // Tag for restart, don't try to optimise
            ; .qword ->restart_code    // Point to above code
            ; .qword 0                 // Ignored
            ; .qword 0                 // Ignored
        );
    }

    fn emit_closure_table(&mut self) {
        // This table contains a struct:
        //
        // Call count/status:
        // +ve = call count
        // -1  = restart, don't optimise
        // -2  = use optimised C version
        //
        // The fields are
        // 0x00 qword call count/status (see above)
        // 0x08 qword address to use (either bytecode/optimised)
        // 0x10 dword section number
        // 0x14 dword bytecode offset
        // 0x18 qword required extra args

        let mut closures = HashMap::new();

        // To make borrow checker happy, do a swap
        std::mem::swap(&mut closures, &mut self.closures);

        for closure in closures.values() {
            let bca = self.get_label(&BytecodeRelativeOffset(closure.bytecode_addr));
            oc_dynasm!(self.ops
                ; =>closure.label
                ; .qword 0       // call count
                ; .qword =>bca   // bytecode addr
                ; .dword self.section_number as _
                ; .dword closure.bytecode_addr as _
                ; .qword (closure.arity - 1) as _
            );
        }
    }

    fn emit_process_events_shared(&mut self) {
        /* process_events - calling convention - put return address in rsi */
        oc_dynasm!(self.ops
            ; ->process_events:
        );

        self.emit_event(b"process_events\0");

        oc_dynasm!(self.ops
            // Setup_for_event
            ; mov rax, mlvalues::LongValue::UNIT.0 as i32
            ; sub r_sp, 6 * 8           // Push frame
            ; mov [r_sp], r_accu        // Accu
            ; mov [r_sp + 8], rax       // Val_unit
            ; mov [r_sp + 2 * 8], rax   // Val_unit
            ; mov [r_sp + 3 * 8], rsi   // Saved pc (from above LEA)
            ; mov [r_sp + 4 * 8], r_env // Env
            ; mov rax, r_extra_args
            ; add rax, rax
            ; inc rax
            ; mov [r_sp + 5 * 8], rax   // Val_long(extra_args)
            ; mov rsi, QWORD get_extern_sp_addr() as usize as i64
            ; mov [rsi], r_sp           // Save extern sp

            // Process the pending actions
            ; mov rax, QWORD caml_process_pending_actions as i64
            ; call rax

            // Restore_after_event
            ; mov rsi, QWORD get_extern_sp_addr() as usize as i64
            ; mov r_sp, [rsi]                    // Get extern_sp
            ; mov r_accu, [r_sp]                 // Restore accu
            ; mov rax, [r_sp + 3 * 8]            // Save pc for later jumping
            ; mov r_env, [r_sp + 4 * 8]          // Restore env
            ; mov r_extra_args, [r_sp + 5 * 8]   // Restore extra_args
            ; shr r_extra_args, 1                // Long_val(extra_args)
            ; add r_sp, 6 * 8                    // Pop frame
            ; jmp rax                            // Jump to the next pc
        );
    }

    fn setup_for_c_call(&mut self) {
        // Trashes rax, moves OCaml stack down by 2 words
        oc_dynasm!(self.ops
            ; sub r_sp, 16
            ; mov [r_sp], r_env
            ; lea rax, [<instr]
            ; mov [r_sp + 8], rax
            ; mov rax, QWORD get_extern_sp_addr() as usize as i64
            ; mov [rax], r_sp
        );
    }

    fn restore_after_c_call(&mut self) {
        // Trashes rax, OCaml stack up by 2 words
        oc_dynasm!(self.ops
            ; mov rax, QWORD get_extern_sp_addr() as usize as i64
            ; mov r_sp, [rax]
            ; mov r_env, [r_sp]
            ; add r_sp, 16
        );
    }

    // IMPORTANT: remember to add a trailing null byte
    fn emit_fatal_error(&mut self, message: &'static [u8]) {
        let message = CStr::from_bytes_with_nul(message).unwrap();

        oc_dynasm!(self.ops
            ; mov rax, QWORD fatal_message as i64
            ; mov rdi, QWORD message.as_ptr() as i64
            ; call rax
        );
    }

    fn emit_event(&mut self, message: &'static [u8]) {
        if self.compiler_options.print_traces {
            let message = CStr::from_bytes_with_nul(message).unwrap();

            oc_dynasm!(self.ops
                ; push rsi
                ; sub rsp, 8
                ; mov rdi, QWORD message.as_ptr() as i64
                ; mov rsi, r_accu
                ; mov rdx, r_env
                ; mov rcx, r_extra_args
                ; mov r8, r_sp
                ; mov rax, QWORD event_trace as i64
                ; call rax
                ; add rsp, 8
                ; pop rsi
            );
        }
    }
}
