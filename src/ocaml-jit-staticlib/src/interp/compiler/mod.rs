use dynasmrt::x64::Assembler;
use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi, DynamicLabel};
use ocaml_jit_shared::Instruction;
use crate::caml::mlvalues;
use crate::caml::misc::fatal_error;
use std::ops::Deref;

pub fn compile(instructions: &Vec<Instruction<usize>>) {
    let mut ops = Assembler::new().unwrap();
    let mut labels = vec![None; instructions.len()];
    let mut cc = CompilerContext {
        ops,
        labels
    };

    for (offset, instruction) in instructions.iter().enumerate() {
        cc.emit_instruction(instruction, offset);
    }

    let mut ops = cc.ops;
    let buf = ops.finalize().unwrap();

    std::fs::write("/tmp/thing", buf.deref());
}

struct CompilerContext {
    ops: Assembler,
    labels: Vec<Option<DynamicLabel>>,
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

// Support functions

extern "C" {
    fn jit_support_get_field(base: i64, field: i64) -> i64;
    fn jit_support_check_stacks(sp: i64) -> i64;
    fn jit_support_appterm_stacks(nargs: i64, slotsize: i64, sp: i64) -> i64;
    fn jit_support_closure(state: i64, nargs: i64, codeval: i64);
}

impl CompilerContext {
    fn get_label(&mut self, offset: usize) -> DynamicLabel {
        let label_ref = &mut self.labels[offset];
        match label_ref {
            Some(l) => *l,
            None => {
                let label = self.ops.new_dynamic_label();
                *label_ref = Some(label);
                label
            }
        }
    }

    fn emit_instruction(&mut self, instruction: &Instruction<usize>, offset: usize) -> Option<()> {
        let label = self.get_label(offset);
        oc_dynasm!(self.ops
            ; =>label
            ; mov rdi, QWORD offset as i64
            ; mov rax, QWORD do_trace as i64
            ; call rax
        );
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
                    ; mov rsi, field_no as i32
                    ; mov rax, QWORD jit_support_get_field as i64
                    ; call rax
                    ; mov r_accu, rax
                );
            }
            Instruction::Push => {
                // *--sp = accu
                oc_dynasm!(self.ops
                    ; mov [r_sp], r_accu
                    ; sub r_sp, 8
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
                let unit_v: i64 = mlvalues::LongValue::UNIT.into();
                oc_dynasm!(self.ops
                    ; mov [r_sp + offset], r_accu
                    ; mov r_accu, unit_v as i32
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
            Instruction::Apply(1) => {
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
            },
            Instruction::Apply(2) => {
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
            Instruction::Apply(3) => {
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
            /*Instruction::Return(_) => {}
            Instruction::Restart => {}
            Instruction::Grab(_) => {}
            */
            Instruction::Closure(codeval, nargs) => {
                let nargs = *nargs as i32;
                let label = self.get_label(*codeval);
                oc_dynasm!(self.ops
                    ; mov rdi, rsp
                    ; mov rsi, nargs
                    ; lea rdx, [=>label]
                    ; push r_accu
                    ; push r_env
                    ; push r_sp
                    ; push r_extra_args
                    ; mov rax, QWORD jit_support_closure as i64
                    ; call rax
                    ; pop r_extra_args
                    ; pop r_sp
                    ; pop r_env
                    ; pop r_accu
                );
            }
            /*
            Instruction::ClosureRec(_, _) => {}
            Instruction::OffsetClosure(_) => {}
            Instruction::GetGlobal(_) => {}
            Instruction::SetGlobal(_) => {}
            Instruction::Const(_) => {}
            Instruction::MakeBlock(_, _) => {}
            Instruction::MakeFloatBlock(_) => {}
            Instruction::GetField(_) => {}
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
            /*
            Instruction::BranchIf(_) => {}
            Instruction::BranchIfNot(_) => {}
            Instruction::Switch(_, _) => {}
            Instruction::BoolNot => {}
            Instruction::PushTrap(_) => {}
            Instruction::PopTrap => {}
            Instruction::Raise(_) => {}
            Instruction::CheckSignals => {}
            Instruction::CCall1(_) => {}
            Instruction::CCall2(_) => {}
            Instruction::CCall3(_) => {}
            Instruction::CCall4(_) => {}
            Instruction::CCall5(_) => {}
            Instruction::CCallN(_, _) => {}
            Instruction::ArithInt(_) => {}
            Instruction::IntCmp(_) => {}
            Instruction::OffsetInt(_) => {}
            Instruction::OffsetRef(_) => {}
            Instruction::IsInt => {}
            Instruction::GetMethod => {}
            Instruction::GetPubMet(_, _) => {}
            Instruction::GetDynMet => {}
            Instruction::Stop => {}
            Instruction::Break => {}
            Instruction::Event => {}*/
            _ => {
                oc_dynasm!(self.ops
                    ; mov rax, QWORD unimplemented as i64
                    ; call rax
                );
            }
        }

        Some(())
    }
}

extern "C" fn unimplemented() {
    fatal_error("Unimplemented!");
}

fn do_trace(pc: i64) {
    println!("Trace {}", pc);
}