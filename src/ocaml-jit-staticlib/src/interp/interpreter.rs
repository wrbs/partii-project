use crate::caml::config::MAX_YOUNG_WOSIZE;
use crate::caml::mlvalues::{BlockValue, Color, Header, LongValue, Tag, Value};
use crate::caml::{domain_state, fail, memory, prims};
use crate::interp::GlobalData;
use ocaml_jit_shared::{ArithOp, Instruction};

const TRACE: bool = true;

pub fn interpret(code: &[i32]) -> Value {
    if code.as_ptr() as usize == 0x0 {
        // This is what OCaml does to initialise the interpreter, but we're already set up
        return LongValue::UNIT.into();
    }

    let mut pc = GlobalData::get().translate_pc_exn(code.as_ptr());
    let mut accu: Value = LongValue::UNIT.into();

    loop {
        let mut new_pc = None; // None (default) = increment, Some(x) = branch to x

        let instruction = &GlobalData::get().instructions[pc];

        if TRACE {
            println!(
                "PC={:5} SP={:5} ACCU={:16X}\t{:?}",
                pc,
                domain_state::stack_size(),
                accu.0,
                instruction
            );
        }

        match instruction {
            Instruction::Acc(i) => {
                accu = peek(*i as isize);
            }
            // Instruction::EnvAcc(_) => {},
            Instruction::Push => {
                push(accu);
            }
            Instruction::Pop(n) => {
                let n = *n as isize;
                stack_shift(-n);
            }
            // Instruction::Assign(_) => {},
            // Instruction::PushRetAddr(_) => {},
            // Instruction::Apply(_) => {},
            // Instruction::ApplyTerm(_, _) => {},
            // Instruction::Return(_) => {},
            // Instruction::Restart => {},
            // Instruction::Grab(_) => {},
            Instruction::Closure(funcptr, nvars) => {
                let funcptr = Value(*funcptr as i64);
                let nvars = *nvars as usize;

                if nvars > 0 {
                    push(accu);
                }

                let block = if nvars < MAX_YOUNG_WOSIZE {
                    let block = memory::alloc_small(1 + nvars, Tag::CLOSURE);
                    for i in 0..nvars {
                        block.set_field_small(i + 1, pop());
                    }
                    block
                } else {
                    let block = memory::alloc_shr(1 + nvars, Tag::CLOSURE);
                    for i in 0..nvars {
                        block.initialize_field(i + 1, pop());
                    }
                    block
                };

                // code pointer not in heap, so doesn't use initialize
                block.set_field_small(0, funcptr);

                accu = block.into()
            }
            Instruction::ClosureRec(funcs, nvars) => {
                let nvars = *nvars as usize;
                let nfuncs = funcs.len();
                let blksize = funcs.len() * 2 - 1 + nvars;

                if nvars > 0 {
                    push(accu);
                }
                let offset = nfuncs * 2 - 1;

                let block = if blksize < MAX_YOUNG_WOSIZE {
                    let block = memory::alloc_small(blksize, Tag::CLOSURE);
                    for i in 0..nvars {
                        block.set_field_small(offset + i, peek(i as isize))
                    }
                    block
                } else {
                    // we have to use caml_initialize instead
                    let block = memory::alloc_shr(blksize, Tag::CLOSURE);
                    for i in 0..nvars {
                        block.initialize_field(offset + i, peek(i as isize))
                    }
                    block
                };
                stack_shift(nvars as isize);

                debug_assert!(funcs.len() > 0);
                // Regardless, the other stuff doesn't need to go through caml initialize
                block.set_field_small(0, Value(funcs[0] as i64));

                push(block.into());

                for i in 1..nfuncs {
                    let first_fieldno = 1 + (i - 1) * 2;
                    // Color irrelevant apparently
                    block.set_field_small(
                        first_fieldno,
                        Header::make(i * 2, Tag::INFIX, Color::White).into(),
                    );
                    let val = Value(funcs[i] as i64);
                    block.set_field_small(first_fieldno + 1, val);
                    push(val);
                }

                accu = block.into();
            }
            // Instruction::OffsetClosure(_) => {},
            Instruction::GetGlobal(index) => {
                let index = *index as usize;
                accu = BlockValue::globals().get_field(index);
            }
            Instruction::SetGlobal(index) => {
                let index = *index as usize;
                BlockValue::globals().modify_field(index, accu);
                accu = LongValue::UNIT.into();
            }
            Instruction::Const(n) => accu = LongValue::from(*n as i64).into(),
            Instruction::MakeBlock(size, tag) => {
                let tag = Tag(*tag);
                let wosize = *size as usize;

                if wosize == 0 {
                    accu = BlockValue::atom(tag);
                } else if wosize < MAX_YOUNG_WOSIZE {
                    let block = memory::alloc_small(wosize, tag);
                    block.set_field_small(0, accu);
                    for i in 1..wosize {
                        block.set_field_small(i, pop());
                    }
                    accu = block.into();
                } else {
                    let block = memory::alloc_shr(wosize, tag);
                    block.initialize_field(0, accu);
                    for i in 1..wosize {
                        block.initialize_field(i, pop());
                    }
                    accu = block.into();
                }
            }
            // Instruction::MakeFloatBlock(_) => {},
            Instruction::GetField(n) => {
                accu = accu.as_block().get_field(*n as usize);
            }
            Instruction::SetField(n) => {
                accu.as_block().modify_field(*n as usize, pop());
            }
            // Instruction::GetFloatField(_) => {},
            // Instruction::SetFloatField(_) => {},
            // Instruction::VecTLength => {},
            // Instruction::GetVecTItem => {},
            // Instruction::SetVecTItem => {},
            // Instruction::GetStringChar => {},
            // Instruction::GetBytesChar => {},
            // Instruction::SetBytesChar => {},
            Instruction::Branch(to) => {
                new_pc = Some(*to);
            }
            // Instruction::BranchIf(_) => {},
            // Instruction::BranchIfNot(_) => {},
            // Instruction::Switch(_, _) => {},
            // Instruction::BoolNot => {},
            // Instruction::PushTrap(_) => {},
            // Instruction::PopTrap => {},
            // Instruction::Raise(_) => {},
            // Instruction::CheckSignals => {},
            Instruction::CCall1(primno) => {
                accu = unsafe { prims::call_prim_1(*primno as usize, accu) };
            }
            Instruction::CCall2(primno) => {
                accu = unsafe { prims::call_prim_2(*primno as usize, accu, peek(2)) };
                stack_shift(-1);
            }
            Instruction::CCall3(primno) => {
                accu = unsafe { prims::call_prim_3(*primno as usize, accu, peek(2), peek(3)) };
                stack_shift(-2);
            }
            Instruction::CCall4(primno) => {
                accu = unsafe {
                    prims::call_prim_4(*primno as usize, accu, peek(2), peek(3), peek(4))
                };
                stack_shift(-3);
            }
            Instruction::CCall5(primno) => {
                accu = unsafe {
                    prims::call_prim_5(*primno as usize, accu, peek(2), peek(3), peek(4), peek(5))
                };
                stack_shift(-4);
            }
            Instruction::CCallN(nargs, primno) => {
                let nargs = *nargs;
                let pointer = unsafe { domain_state::get_sp().add(2) };
                accu = unsafe { prims::call_prim_n(*primno as usize, pointer, nargs) };
                stack_shift(nargs as isize);
            }
            Instruction::ArithInt(op) => {
                if op == &ArithOp::Neg {
                    let x: i64 = accu.as_long().into();
                    accu = LongValue::from(x).into();
                } else {
                    let a: i64 = accu.as_long().into();
                    let b: i64 = pop().as_long().into();
                    let result = match op {
                        ArithOp::Neg => unreachable!(),
                        ArithOp::Add => a.wrapping_add(b),
                        ArithOp::Sub => a.wrapping_sub(b),
                        ArithOp::Mul => a.wrapping_mul(b),
                        ArithOp::Div => {
                            if b == 0 {
                                fail::raise_zero_divide();
                            }
                            a.wrapping_div(b)
                        }
                        ArithOp::Mod => a.wrapping_rem(b),
                        ArithOp::And => a & b,
                        ArithOp::Or => a | b,
                        ArithOp::Xor => a ^ b,
                        ArithOp::Lsl => a.wrapping_shl(b as u32),
                        ArithOp::Lsr => {
                            ((accu.as_long().0 as u64).wrapping_shr((b as u32).wrapping_add(1)) | 1)
                                as i64
                        }
                        ArithOp::Asr => a.wrapping_shr(b as u32),
                    };

                    accu = LongValue::from(result).into();
                }
            }
            // Instruction::IntCmp(_) => {},
            // Instruction::BranchCmp(_, _, _) => {},
            Instruction::OffsetInt(n) => {
                let x: i64 = accu.as_long().into();
                accu = LongValue::from(x + *n as i64).into();
            }
            // Instruction::OffsetRef(_) => {},
            // Instruction::IsInt => {},
            // Instruction::GetMethod => {},
            // Instruction::GetPubMet(_, _) => {},
            // Instruction::GetDynMet => {},
            // Instruction::Stop => {},
            // Instruction::Break => {},
            // Instruction::Event => {},
            x => panic!("Unknown instruction {:?}", x),
        }

        match new_pc {
            None => pc += 1,
            Some(x) => pc = x,
        }
    }
}

// +ve grows the stack, -ve shrinks the stack
fn stack_shift(amount: isize) -> *mut Value {
    unsafe {
        let sp = domain_state::get_sp();
        let new_sp = sp.offset(-amount);
        domain_state::set_sp(new_sp);
        new_sp
    }
}

fn push(x: Value) {
    let new_sp = stack_shift(1);
    unsafe {
        *new_sp = x;
    }
}

fn pop() -> Value {
    unsafe {
        let val = *domain_state::get_sp();
        stack_shift(-1);
        val
    }
}

// +ve - into past, -ve into future
fn peek(n: isize) -> Value {
    unsafe {
        let sp = domain_state::get_sp();
        *sp.offset(n)
    }
}
