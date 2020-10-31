use super::c_primitives::*;
use super::saved_data::EntryPoint;
use crate::caml::domain_state::get_extern_sp_addr;
use crate::caml::misc::fatal_error;
use crate::caml::mlvalues::{BlockValue, LongValue, Tag, Value};
use crate::caml::{domain_state, mlvalues};
use crate::compiler::saved_data::LongjmpHandler;
use crate::compiler::LongjmpEntryPoint;
use crate::global_data::GlobalData;
use crate::trace::{print_bytecode_trace, print_instruction_trace};
use dynasmrt::x64::Assembler;
use dynasmrt::{dynasm, AssemblyOffset, DynamicLabel, DynasmApi, DynasmLabelApi, ExecutableBuffer};
use ocaml_jit_shared::{ArithOp, BytecodeRelativeOffset, Comp, Instruction, ParsedRelativeOffset};

pub fn compile_instructions(
    section_number: usize,
    instructions: &[Instruction<ParsedRelativeOffset>],
    bytecode_offsets: &[Option<BytecodeRelativeOffset>],
    code: &[i32],
    print_traces: bool,
) -> (ExecutableBuffer, EntryPoint) {
    let ops = Assembler::new().unwrap();

    let labels = vec![None; instructions.len()];

    let mut cc = CompilerContext {
        ops,
        labels,
        print_traces,
        section_number,
    };

    let entrypoint_offset = cc.emit_entrypoint();

    for (offset, instruction) in instructions.iter().enumerate() {
        cc.emit_instruction(
            instruction,
            ParsedRelativeOffset(offset),
            bytecode_offsets[offset].map(|x| unsafe { code.as_ptr().offset(x.0 as isize) }),
        );
    }

    let ops = cc.ops;
    let buf = ops.finalize().unwrap();

    let entrypoint: EntryPoint = unsafe { std::mem::transmute(buf.ptr(entrypoint_offset)) };

    (buf, entrypoint)
}

struct CompilerContext {
    ops: Assembler,
    labels: Vec<Option<DynamicLabel>>,
    print_traces: bool,
    section_number: usize,
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
    fn get_label(&mut self, offset: ParsedRelativeOffset) -> DynamicLabel {
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

    #[allow(clippy::fn_to_numeric_cast)]
    fn emit_entrypoint(&mut self) -> AssemblyOffset {
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

        offset
    }

    #[allow(clippy::fn_to_numeric_cast)]
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

    #[allow(clippy::fn_to_numeric_cast)]
    fn emit_instruction(
        &mut self,
        instruction: &Instruction<ParsedRelativeOffset>,
        offset: ParsedRelativeOffset,
        bytecode_pointer: Option<*const i32>,
    ) -> Option<()> {
        let label = self.get_label(offset);

        oc_dynasm!(self.ops
            ; =>label
        );

        if self.print_traces {
            if let Some(bytecode_pointer) = bytecode_pointer {
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

            oc_dynasm!(self.ops
                ; mov rdi, QWORD offset.0 as i64
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
                    ; mov rdi, r_env
                    ; mov esi, field_no as i32
                    // TODO this doesn't need a function
                    ; mov rax, QWORD jit_support_get_field as i64
                    ; call rax
                    ; mov r_accu, rax
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
                let return_label = self.get_label(*offset);
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
                    ; lea rcx, [>retloc]
                );
                // Push the return frame (retaddr, num_args, env)
                oc_pushretaddr!(self.ops, 8, rcx);
                oc_dynasm!(self.ops
                    // Set the env and extra_args to the new appropriate values
                    ; mov r_env, r_accu
                    ; mov r_extra_args, 0
                    // Check stacks
                    ; mov rdi, r_sp
                    ; mov rax, QWORD jit_support_check_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                    // Done
                    // Get the code value (new pc) of the current accu in rax
                    ; mov rax, [r_accu]
                    ; jmp rax
                    // Define the label used earlier
                    ; retloc:
                );
            }
            Instruction::Apply2 => {
                // Like for Apply(1), but saving two args
                oc_dynasm!(self.ops
                    ; mov rax, [r_sp]
                    ; mov rcx, [r_sp + 8]
                    ; sub r_sp, 24
                    ; mov [r_sp], rax
                    ; mov [r_sp + 8], rcx
                    ; lea rcx, [>retloc]
                );
                oc_pushretaddr!(self.ops, 16, rcx);
                oc_dynasm!(self.ops
                    ; mov r_env, r_accu
                    ; mov r_extra_args, 1
                    // Check stacks
                    ; mov rdi, r_sp
                    ; mov rax, QWORD jit_support_check_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                    // Done
                    ; mov rax, [r_accu]
                    ; jmp rax
                    ; retloc:
                );
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
                    ; lea rcx, [>retloc]
                );
                oc_pushretaddr!(self.ops, 24, rcx);
                oc_dynasm!(self.ops
                    ; mov r_env, r_accu
                    ; mov r_extra_args, 2
                    // Check stacks
                    ; mov rdi, r_sp
                    ; mov rax, QWORD jit_support_check_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                    // Done
                    ; mov rax, [r_accu]
                    ; jmp rax
                    ; retloc:
                );
            }
            Instruction::Apply(n) => {
                // In any other cases the retaddr is already pushed
                // So just set extra args, pc and jump to the closure's pc
                let new_extra_args = (*n - 1) as i32;
                oc_dynasm!(self.ops
                    ; mov r_env, r_accu
                    ; mov r_extra_args, new_extra_args
                    // Check stacks
                    ; mov rdi, r_sp
                    ; mov rax, QWORD jit_support_check_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                    // Done
                    // Get codeval and jump to it
                    ; mov rax, [r_accu]
                    ; jmp rax
                );
            }
            Instruction::ApplyTerm(nargs, slotsize) => {
                let nargs = *nargs as i32;
                let slotsize = *slotsize as i32;
                // for now we're calling into C for the offset
                oc_dynasm!(self.ops
                    ; mov rdi, nargs
                    ; mov rsi, slotsize
                    ; mov rdx, r_sp
                    // Also does check_stacks
                    ; mov rax, QWORD jit_support_appterm_stacks as i64
                    ; call rax
                    ; mov r_sp, rax
                    ; add r_extra_args, nargs - 1
                    ; mov r_env, r_accu
                    // Get codeval and jump to it
                    ; mov rax, [r_accu]
                    ; jmp rax
                );
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
                    ; mov r_env, r_accu
                    ; mov rax, [r_accu]
                    ; jmp rax
                );
            }
            Instruction::Restart => {
                oc_dynasm!(self.ops
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
                );
            }
            Instruction::Grab(required_arg_count) => {
                let prev_restart = self.get_label(ParsedRelativeOffset(offset.0 - 1));
                oc_dynasm!(self.ops
                    ; mov rax, *required_arg_count as i32
                    // If extra_args >= required
                    ; cmp r_extra_args, rax
                    ; jl >re_closure
                    // extra_args -= required
                    ; sub r_extra_args, rax
                    ; jmp >next

                    // Otherwise something more complicated - leave for now
                    ; re_closure:
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; lea rsi, [=>prev_restart]
                    ; mov rax, QWORD jit_support_grab_closure as i64
                    ; call rax
                    ; pop r_accu
                    ; pop r_env
                    ; pop r_sp
                    ; pop r_extra_args
                    ; jmp rax
                    ; next:
                );
            }
            Instruction::Closure(codeval, nargs) => {
                let nargs = *nargs as i32;
                let label = self.get_label(*codeval);
                oc_dynasm!(self.ops
                    ; push r_extra_args
                    ; push r_sp
                    ; push r_env
                    ; push r_accu
                    ; mov rdi, rsp
                    ; mov rsi, nargs
                    ; lea rdx, [=>label]
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
                    let func = self.get_label(*offset);
                    oc_dynasm!(self.ops
                        ; lea rax, [=>func]
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
                    ; mov esi, *field_no as i32
                    ; mov rax, QWORD jit_support_get_field as i64
                    ; call rax
                    ; mov r_accu, rax
                );
            }
            Instruction::SetGlobal(field_no) => {
                oc_dynasm!(self.ops
                    ; mov rax, QWORD get_global_data_addr()
                    ; mov rdi, [rax]
                    ; mov esi, *field_no as i32
                    ; mov rdx, r_accu
                    ; mov rax, QWORD jit_support_set_field as i64
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
            // Instruction::MakeFloatBlock(_) => {}
            Instruction::GetField(field_no) => {
                oc_dynasm!(self.ops
                    ; mov rdi, r_accu
                    ; mov esi, *field_no as i32
                    // TODO this doesn't need a function
                    ; mov rax, QWORD jit_support_get_field as i64
                    ; call rax
                    ; mov r_accu, rax
                );
            }
            /*
            Instruction::SetField(_) => {}
            Instruction::GetFloatField(_) => {}
            Instruction::SetFloatField(_) => {}
            Instruction::VecTLength => {}
            Instruction::GetVecTItem => {}
            Instruction::SetVecTItem => {}
            Instruction::GetStringChar => {}
            Instruction::GetBytesChar => {}
            Instruction::SetBytesChar => {}
            */
            Instruction::Branch(loc) => {
                let label = self.get_label(*loc);
                oc_dynasm!(self.ops
                    ; jmp =>label
                );
            }
            Instruction::BranchIf(loc) => {
                let label = self.get_label(*loc);
                oc_dynasm!(self.ops
                    ; cmp r_accu, BYTE 1 // Which is Val_false
                    ; je >next
                    ; jmp =>label
                    ; next:
                );
            }
            Instruction::BranchIfNot(loc) => {
                let label = self.get_label(*loc);
                oc_dynasm!(self.ops
                    ; cmp r_accu, BYTE 1 // Which is Val_false
                    ; jne >next
                    ; jmp =>label
                    ; next:
                );
            }
            Instruction::Switch(ints, blocks) => {
                // Really inefficient for now - in the future I'll work out how to do a jump table
                for (i, offset) in ints.iter().enumerate() {
                    let label = self.get_label(*offset);
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
                    let label = self.get_label(*offset);
                    oc_dynasm!(self.ops
                        ; cmp al, tag as i8
                        ; je =>label
                    );
                }

                self.unreachable();
            }
            /*
            Instruction::BoolNot => {}
            */
            Instruction::PushTrap(loc) => {
                let label = self.get_label(*loc);
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
                oc_dynasm!(self.ops
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
            /*
            Instruction::CheckSignals => {}
            */
            Instruction::CCall1(primno) => {
                // FIXME Setup_for_c_call
                // TODO - possible optimisation, could load the static address
                // if it's currently in the table
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
                oc_dynasm!(self.ops
                    ; mov rdi, *primno as i32
                    ; mov rax, QWORD jit_support_get_primitive as i64
                    ; call rax
                    ; mov rdi, r_accu
                    ; mov rsi, [r_sp]
                    ; mov rdx, [r_sp + 8]
                    ; mov rcx, [r_sp + 16]
                    ; mov r8, [r_sp + 24]
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
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
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
                self.setup_for_c_call(offset);
                oc_dynasm!(self.ops
                    ; mov rax, QWORD caml_raise_zero_divide as i64
                    ; call rax
                    ; next:
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
            /*
            Instruction::ArithInt(_) => {}
            */
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
                            ; cmova r_accu, rdi
                        );
                    }
                }
            }
            Instruction::BranchCmp(cmp, i, l) => {
                let label = self.get_label(*l);
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
                            ; ja =>label
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
            /*
            Instruction::OffsetRef(_) => {}
            */
            Instruction::IsInt => {
                oc_dynasm!(self.ops
                    ; and r_accu, 1
                    ; shl r_accu, 1
                    ; add r_accu, 1
                );
            }
            /*
            Instruction::GetMethod => {}
            Instruction::GetPubMet(_, _) => {}
            Instruction::GetDynMet => {}
            */
            Instruction::Stop => {
                // Call the function so that the entrypoint and code that uses it is visually nearby
                // for easier changes
                oc_dynasm!(self.ops
                    // Set some global variables
                    ; mov rax, QWORD jit_support_stop as i64
                    ; mov rdi, [rsp]
                    ; mov rsi, r_sp
                    ; call rax
                    // Set the return value
                    ; mov rax, r_accu
                );
                self.emit_return();
            }
            /*
            Instruction::Break => {}
            Instruction::Event => {}*/
            _ => self.unimplemented(),
        }

        Some(())
    }

    fn setup_for_c_call(&mut self, offset: ParsedRelativeOffset) {
        // Trashes rax, moves OCaml stack down by 2 words
        let next_instr = self.get_label(ParsedRelativeOffset(offset.0 + 1));
        oc_dynasm!(self.ops
            ; sub r_sp, 16
            ; mov [r_sp], r_env
            ; lea rax, [=>next_instr]
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

    fn unimplemented(&mut self) {
        oc_dynasm!(self.ops
            ; mov rax, QWORD unimplemented as i64
            ; call rax
        );
    }

    fn unreachable(&mut self) {
        oc_dynasm!(self.ops
            ; mov rax, QWORD unreachable as i64
            ; call rax
        );
    }
}

extern "C" fn unreachable() {
    // fatal_error("Should be unreachable!");
}

extern "C" fn unimplemented() {
    fatal_error("Unimplemented!");
}

extern "C" fn bytecode_trace(
    pc: *const i32,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
) {
    let global_data = GlobalData::get();
    print_bytecode_trace(&global_data, pc, accu, env, extra_args, sp);
}

extern "C" fn instruction_trace(
    pc: i64,
    accu: u64,
    env: u64,
    extra_args: u64,
    sp: *const Value,
    section_number: u64,
) {
    let global_data = GlobalData::get();
    let section = global_data.compiler_data.sections[section_number as usize]
        .as_ref()
        .expect("Section already released");
    let instruction = &section.instructions[pc as usize];
    print_instruction_trace(&global_data, instruction, accu, env, extra_args, sp);
}
