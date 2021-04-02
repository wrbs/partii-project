use std::{
    collections::{hash_map::VacantEntry, HashMap},
    fmt::Write,
};

use crate::{
    basic_blocks::{BasicBlock, BasicBlockExit, BasicBlockInstruction, BasicClosure},
    instructions::{ArithOp, Comp},
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use codegen::{
    binemit::{StackMap, StackMapSink},
    ir::{FuncRef, GlobalValue, Inst},
    print_errors::pretty_error,
};
use cranelift::{frontend::Switch, prelude::*};
use cranelift_module::{DataId, FuncId, Linkage, Module, ModuleError};
use primitives::{CLOSURE_TAG, INFIX_TAG, MAX_YOUNG_WOSIZE};
use types::{I8, I32, I64, R64};

use self::primitives::{CamlStateField, CraneliftPrimitiveFunction, CraneliftPrimitiveValue};

#[cfg(test)]
mod test;

pub mod primitives;

#[derive(Debug)]
pub enum CompilationResult {
    UnsupportedClosure,
    SupportedClosure(FuncId),
}

#[derive(Debug, Default)]
pub struct CompilerOutput {
    ir_after_codegen: String,
    ir_after_compile: String,
    disasm: String,
}

pub struct CraneliftCompilerOptions {
    pub use_call_traces: bool,
}

pub struct CraneliftCompiler<M: Module> {
    pub module: M,
    ctx: codegen::Context,
    function_builder_context: FunctionBuilderContext,
    primitives: Primitives,
}

// A function that can be used to lookup closures
pub trait LookupClosureCode: Fn(usize) -> Option<*const u8> {}
impl<T> LookupClosureCode for T where T: Fn(usize) -> Option<*const u8> {}

#[derive(Debug)]
struct StackMaps<'a> {
    maps: &'a mut Vec<(u32, StackMap)>,
}

impl<'a> StackMapSink for StackMaps<'a> {
    fn add_stack_map(&mut self, offset: codegen::binemit::CodeOffset, map: StackMap) {
        self.maps.push((offset, map));
    }
}
pub fn format_c_call_name(id: usize) -> String {
    format!("oc_prim{}", id)
}

impl<M> CraneliftCompiler<M>
where
    M: Module,
{
    pub fn new(mut module: M) -> Result<Self> {
        let ctx = module.make_context();
        let function_builder_context = FunctionBuilderContext::new();


        Ok(Self {
            module,
            ctx,
            function_builder_context,
            primitives: Primitives::default(),
        })
    }

    pub fn compile_closure<F: LookupClosureCode>(
        &mut self,
        func_name: &str,
        closure: &BasicClosure,
        lookup_closure_code: F,
        options: &CraneliftCompilerOptions,
        mut debug_output: Option<&mut CompilerOutput>,
        stack_maps: &mut Vec<(u32, StackMap)>,
    ) -> Result<CompilationResult> {
        if closure.has_trap_handlers {
            return Ok(CompilationResult::UnsupportedClosure);
        }

        self.module.clear_context(&mut self.ctx);

        // First arg -env
        self.ctx.func.signature.params.push(AbiParam::new(R64));
        // Second arg - current SP
        self.ctx.func.signature.params.push(AbiParam::new(I64));

        // Ret 1 = return value of closure / what closure to apply if tail-calling
        self.ctx.func.signature.returns.push(AbiParam::new(R64));
        // Ret 2 = new extra args after function (will turn into a tail call if > 0)
        self.ctx.func.signature.returns.push(AbiParam::new(I64));

        let func_id =
            self.module
                .declare_function(func_name, Linkage::Export, &self.ctx.func.signature)?;

        // Compile the function
        // TODO - share this once I stop having errors that stop it from being automatically cleared
        self.function_builder_context = FunctionBuilderContext::new();
        let mut translator = self.create_translator(closure, options, lookup_closure_code)?;
        for basic_block in &closure.blocks {
            translator.translate_block(basic_block).with_context(|| {
                format!("Problem compiling basic block {}", basic_block.block_id)
            })?;
        }
        translator.finalise()?;

        if let Some(co) = debug_output.as_deref_mut() {
            co.ir_after_codegen.clear();
            write!(
                co.ir_after_codegen,
                "{}",
                self.ctx.func.display(self.module.isa())
            )
            .unwrap();
        }

        // Finalise and compile
        self.ctx.want_disasm = debug_output.is_some();
        let mut stack_map_sink = StackMaps { maps: stack_maps };
        match self.module.define_function(
            func_id,
            &mut self.ctx,
            &mut codegen::binemit::NullTrapSink {},
            &mut stack_map_sink,
        ) {
            Ok(_) => {}
            Err(ModuleError::Compilation(e)) => {
                bail!(
                    "{}",
                    pretty_error(&self.ctx.func, Some(self.module.isa()), e)
                )
            }

            Err(e) => return Err(e.into()),
        };

        if let Some(co) = debug_output {
            co.ir_after_compile.clear();
            co.disasm.clear();

            write!(
                co.ir_after_compile,
                "{}",
                self.ctx.func.display(self.module.isa())
            )
            .unwrap();

            if let Some(disasm) = self
                .ctx
                .mach_compile_result
                .as_ref()
                .and_then(|d| d.disasm.as_ref())
            {
                write!(co.disasm, "{}", disasm).unwrap();
            }
        }


        Ok(CompilationResult::SupportedClosure(func_id))
    }

    fn create_translator<'a, F: LookupClosureCode>(
        &'a mut self,
        closure: &'a BasicClosure,
        options: &'a CraneliftCompilerOptions,
        lookup_closure_code: F,
    ) -> Result<FunctionTranslator<'a, M, F>> {
        let mut builder =
            FunctionBuilder::new(&mut self.ctx.func, &mut self.function_builder_context);
        let blocks: Vec<_> = (0..closure.blocks.len())
            .map(|_| builder.create_block())
            .collect();
        let return_block = builder.create_block();
        let entry_block = blocks[0];
        builder.append_block_params_for_function_params(entry_block);
        builder.seal_block(entry_block);
        builder.switch_to_block(entry_block);

        let mut var_count = 0;
        let mut var = |typ| {
            let var = Variable::new(var_count);
            var_count += 1;
            builder.declare_var(var, typ);
            var
        };

        let acc = var(R64);

        // Overhead allows implementing instructions like Closure by spilling the acc onto the stack
        let to_allocate = closure.max_stack_size + 1;
        let stack_vars: Vec<_> = (0..to_allocate).map(|_| var(R64)).collect();
        let return_extra_args_var = var(I64);
        let sp = var(I64);

        let stack_size = closure.arity;
        let env = builder.block_params(blocks[0])[0];
        let initial_sp = builder.block_params(blocks[0])[1];

        builder.def_var(sp, initial_sp);

        let zero = builder.ins().null(R64);
        let zero_i = builder.ins().iconst(I64, 0);

        let mut ft = FunctionTranslator {
            builder,
            module: &mut self.module,
            lookup_closure_code,
            stack_size,
            stack_vars,
            env,
            acc,
            sp,
            blocks,
            options,
            return_extra_args_var,
            return_block,
            primitives: &mut self.primitives,
            caml_state_addr: zero, // patched later - but we need a ft for the context of the primitive lookup
            used_c_calls: HashMap::new(),
            used_funcs: HashMap::new(),
            used_values: HashMap::new(),
        };

        // This is where we patch it
        let caml_state_addr =
            ft.get_global_variable(I64, CraneliftPrimitiveValue::CamlStateAddr)?;
        ft.caml_state_addr = caml_state_addr;

        // Declare the variables
        let cur_sp = ft.builder.use_var(ft.sp);
        for i in 0..closure.arity {
            let arg = ft
                .builder
                .ins()
                .load(R64, MemFlags::trusted(), cur_sp, 8 * i as i32);
            ft.builder
                .def_var(ft.stack_vars[closure.arity - i - 1], arg);
        }
        let new_sp = ft.builder.ins().iadd_imm(cur_sp, 8 * closure.arity as i64);
        ft.builder.def_var(ft.sp, new_sp);

        // Zero-initialise the other vars
        ft.builder.def_var(acc, zero);
        for i in closure.arity..(closure.max_stack_size as usize) {
            ft.builder.def_var(ft.stack_vars[i], zero);
        }
        ft.builder.def_var(return_extra_args_var, zero_i);


        Ok(ft)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arity {
    N1,
    N2,
    N3,
    N4,
    N5,
    VarArgs(u32),
}

struct CCall {
    id: FuncId,
    arity: Arity,
}

struct FunctionTranslator<'a, M, F>
where
    M: Module,
    F: LookupClosureCode,
{
    builder: FunctionBuilder<'a>,
    module: &'a mut M,
    lookup_closure_code: F,
    stack_size: usize,
    stack_vars: Vec<Variable>,
    options: &'a CraneliftCompilerOptions,
    env: Value,
    acc: Variable,
    return_extra_args_var: Variable,
    sp: Variable,
    // Represents the blocks in my basic block translation
    blocks: Vec<Block>,
    return_block: Block,
    // Primitives
    caml_state_addr: Value,
    primitives: &'a mut Primitives,
    used_values: HashMap<CraneliftPrimitiveValue, GlobalValue>,
    used_funcs: HashMap<CraneliftPrimitiveFunction, FuncRef>,
    used_c_calls: HashMap<u32, FuncRef>,
}

impl<'a, M, F> FunctionTranslator<'a, M, F>
where
    M: Module,
    F: LookupClosureCode,
{
    fn translate_block(&mut self, basic_block: &BasicBlock) -> Result<()> {
        // Start the block
        if basic_block.block_id != 0 {
            self.builder
                .switch_to_block(self.blocks[basic_block.block_id]);
        }
        self.stack_size = basic_block.start_stack_size as usize;

        for instr in &basic_block.instructions {
            self.translate_instruction(instr)
                .with_context(|| format!("Problem translating instruction {:?}", instr))?;
        }

        self.translate_exit(&basic_block.exit)
            .with_context(|| format!("Problem translating exit {:?}", &basic_block.exit))?;

        ensure!(self.stack_size == basic_block.end_stack_size as usize);

        // Seal any blocks this current block is the last predecessor of
        for sealed_block in &basic_block.sealed_blocks {
            self.builder.seal_block(self.blocks[*sealed_block]);
        }

        Ok(())
    }

    fn translate_instruction(&mut self, instruction: &BasicBlockInstruction) -> Result<()> {
        match instruction {
            &BasicBlockInstruction::Acc(i) => {
                let v = self.pick_ref(i)?;
                self.set_acc_ref(v);
            }
            BasicBlockInstruction::EnvAcc(i) => {
                let v = self
                    .builder
                    .ins()
                    .load(R64, MemFlags::trusted(), self.env, 8 * *i as i32);
                self.set_acc_ref(v);
            }
            BasicBlockInstruction::Push => {
                let v = self.get_acc_ref();
                self.push_ref(v)?;
            }
            BasicBlockInstruction::Pop(n) => {
                self.pop(*n)?;
            }
            // BasicBlockInstruction::Assign(_) => {}
            BasicBlockInstruction::Apply1 => {
                self.emit_apply(1)?;
            }
            BasicBlockInstruction::Apply2 => {
                self.emit_apply(2)?;
            }
            BasicBlockInstruction::Apply3 => {
                self.emit_apply(3)?;
            }
            BasicBlockInstruction::PushRetAddr => {
                self.push_dummy(3)?;
            }
            BasicBlockInstruction::Apply(nvars) => {
                let nvars = *nvars as usize;
                ensure!(nvars > 0);
                self.emit_apply(nvars)?;
                self.pop(3)?;
            }
            BasicBlockInstruction::Closure(codeval, nvars) => {
                let nvars = *nvars;
                if nvars > 0 {
                    let accu = self.get_acc_ref();
                    self.push_ref(accu)?;
                }

                let mut vars = vec![];
                vars.push(None); // Will store codeval, but doesn't go through caml_initialize
                for i in 0..nvars {
                    vars.push(Some(self.pick_ref(i)?));
                }
                self.pop(nvars)?;

                let block = self.alloc(&vars, CLOSURE_TAG)?;
                let code_ptr = (self.lookup_closure_code)(*codeval)
                    .ok_or_else(|| anyhow!("Could not find closure {:?}", codeval))?;

                let block_addr = self.ref_to_int(block);
                let code_ptr_loc = self.builder.ins().iconst(I64, code_ptr as i64);
                self.builder
                    .ins()
                    .store(MemFlags::trusted(), code_ptr_loc, block_addr, 0);

                self.set_acc_ref(block);
            }
            BasicBlockInstruction::ClosureRec(funcs, nvars) => {
                let nvars = *nvars;
                let code_pointers_res: Result<Vec<_>> = funcs
                    .iter()
                    .map(|offset| {
                        (self.lookup_closure_code)(*offset)
                            .ok_or_else(|| anyhow!("Could not find closure {:?}", offset))
                    })
                    .collect();

                let code_pointers = code_pointers_res?;

                let mut contents = vec![];
                for _ in 0..(funcs.len() * 2 - 1) {
                    contents.push(None); // We'll fill this in later with infix headers/code pointers
                }

                if nvars > 0 {
                    let accu = self.get_acc_ref();

                    self.push_ref(accu)?;
                    for i in 0..nvars {
                        contents.push(Some(self.pick_ref(i)?));
                    }
                    self.pop(nvars)?;
                }

                let block = self.alloc(&contents, CLOSURE_TAG)?;
                let block_base = self.ref_to_int(block);

                // Now fill in the code pointers
                for (i, code_pointer) in code_pointers.iter().enumerate() {
                    let code_pointer_val = self.builder.ins().iconst(I64, *code_pointer as i64);
                    if i == 0 {
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            code_pointer_val,
                            block_base,
                            0,
                        );
                        self.push_int(block_base)?;
                    } else {
                        let header_val = self
                            .builder
                            .ins()
                            .iconst(I64, make_header(i * 2, INFIX_TAG));
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            header_val,
                            block_base,
                            (2 * i - 1) as i32,
                        );
                        self.builder.ins().store(
                            MemFlags::trusted(),
                            code_pointer_val,
                            block_base,
                            (2 * i) as i32,
                        );
                        let infix = self.builder.ins().iadd_imm(block_base, 2 * i as i64);
                        self.push_int(infix)?;
                    }
                }
            }
            BasicBlockInstruction::MakeBlock(0, tag) => {
                bail!("unimplemented: atom");
            }
            BasicBlockInstruction::MakeBlock(wosize, tag) => {
                let wosize = *wosize as usize;
                let tag = *tag;

                let mut vars = vec![];
                vars.push(Some(self.get_acc_ref()));

                for i in 1..wosize {
                    vars.push(Some(self.pick_ref((i - 1) as u32)?));
                }

                let block = self.alloc(&vars, tag)?;

                self.set_acc_ref(block);
                self.pop(wosize as u32 - 1)?;

                if self.options.use_call_traces {
                    self.call_primitive(CraneliftPrimitiveFunction::MakeBlockTrace, &[block])?;
                }
            }
            // BasicBlockInstruction::MakeFloatBlock(_) => {}
            BasicBlockInstruction::OffsetClosure(i) => {
                let env_i = self.ref_to_int(self.env);
                let result = self.builder.ins().iadd_imm(env_i, *i as i64 * 8);
                self.set_acc_int(result);
            }
            BasicBlockInstruction::GetGlobal(field_no) => {
                let glob_data_addr =
                    self.get_global_variable(I64, CraneliftPrimitiveValue::GlobalDataAddr)?;
                let glob_val = self
                    .builder
                    .ins()
                    .load(I64, MemFlags::trusted(), glob_data_addr, 0);
                let result = self.builder.ins().load(
                    R64,
                    MemFlags::trusted(),
                    glob_val,
                    *field_no as i32 * 8,
                );
                self.set_acc_ref(result);
            }
            // BasicBlockInstruction::SetGlobal(_) => {}
            BasicBlockInstruction::GetField(i) => {
                let accu = self.get_acc_ref();
                let res = self
                    .builder
                    .ins()
                    .load(R64, MemFlags::trusted(), accu, *i as i32 * 8);
                self.set_acc_ref(res);
            }
            // BasicBlockInstruction::SetField(_) => {}
            // BasicBlockInstruction::GetFloatField(_) => {}
            // BasicBlockInstruction::SetFloatField(_) => {}
            // BasicBlockInstruction::GetVecTItem => {}
            // BasicBlockInstruction::SetVecTItem => {}
            // BasicBlockInstruction::GetBytesChar => {}
            // BasicBlockInstruction::SetBytesChar => {}
            // BasicBlockInstruction::OffsetRef(_) => {}
            BasicBlockInstruction::Const(i) => {
                let ml_value = i64_to_value(*i as i64);
                let int_value = self.builder.ins().iconst(I64, ml_value);
                let ref_value = self.int_to_ref(int_value);
                self.set_acc_ref(ref_value);
            }
            BasicBlockInstruction::BoolNot => {
                let val = self.get_acc_int();
                let res = self.builder.ins().bxor_imm(val, 2);
                self.set_acc_int(res);
            }
            BasicBlockInstruction::NegInt => {
                let two = self.builder.ins().iconst(I64, 2);
                let acc = self.get_acc_int();
                let res = self.builder.ins().isub(two, acc);
                self.set_acc_int(res);
            }
            BasicBlockInstruction::ArithInt(op) => {
                let a = self.get_acc_int();
                let b = self.pick_int(0)?;
                self.pop(1)?;

                let result = match op {
                    ArithOp::Add => {
                        // a + b = (x * 2 + 1) + (y * 2 + 1) = (x + y) * 2 + 2
                        // result = a + b - 1 = (x + y) * 2 + 1
                        let added = self.builder.ins().iadd(a, b);
                        self.builder.ins().iadd_imm(added, -1)
                    }
                    ArithOp::Sub => {
                        // It's a - b + 1 for similar reasons to add
                        let subbed = self.builder.ins().isub(a, b);
                        self.builder.ins().iadd_imm(subbed, 1)
                    }
                    ArithOp::Mul => {
                        let al = self.value_to_long(a);
                        let bl = self.value_to_long(b);

                        let rl = self.builder.ins().imul(al, bl);
                        self.long_to_value(rl)
                    }
                    ArithOp::Div => {
                        self.check_div_zero(b)?;
                        let al = self.value_to_long(a);
                        let bl = self.value_to_long(b);
                        let rl = self.builder.ins().sdiv(al, bl);
                        self.long_to_value(rl)
                    }
                    ArithOp::Mod => {
                        self.check_div_zero(b)?;
                        let al = self.value_to_long(a);
                        let bl = self.value_to_long(b);
                        let rl = self.builder.ins().srem(al, bl);
                        self.long_to_value(rl)
                    }
                    ArithOp::And => self.builder.ins().band(a, b),
                    ArithOp::Or => self.builder.ins().bor(a, b),
                    ArithOp::Xor => {
                        let xor = self.builder.ins().bxor(a, b);
                        self.builder.ins().bor_imm(xor, 1)
                    }
                    ArithOp::Lsl => {
                        // accu = (value)((((intnat) accu - 1) << Long_val(*sp++)) + 1); Next;
                        let shift = self.value_to_long(b);
                        let adec = self.builder.ins().iadd_imm(a, -1);
                        let shifted = self.builder.ins().ishl(adec, shift);
                        self.builder.ins().iadd_imm(shifted, 1)
                    }
                    ArithOp::Lsr => {
                        // accu = (value)((((uintnat) accu) >> Long_val(*sp++)) | 1); Next;
                        let shift = self.value_to_long(b);
                        let shifted = self.builder.ins().ushr(a, shift);
                        self.builder.ins().bor_imm(shifted, 1)
                    }
                    ArithOp::Asr => {
                        // accu = (value)((((intnat) accu) >> Long_val(*sp++)) | 1); Next;
                        let shift = self.value_to_long(b);
                        let shifted = self.builder.ins().sshr(a, shift);
                        self.builder.ins().bor_imm(shifted, 1)
                    }
                };
                self.set_acc_int(result);
            }
            // BasicBlockInstruction::IsInt => {}
            BasicBlockInstruction::IntCmp(cmp) => {
                let x = self.get_acc_int();
                let y = self.pick_int(0)?;
                self.pop(1)?;

                let flags = self.builder.ins().ifcmp(x, y);
                let cc = comp_to_cc(cmp);

                let val_true = self.builder.ins().iconst(I64, 3);
                let val_false = self.builder.ins().iconst(I64, 1);

                // This translates to a cmov which is nice!
                let res = self
                    .builder
                    .ins()
                    .selectif(I64, cc, flags, val_true, val_false);
                self.set_acc_int(res);
            }
            BasicBlockInstruction::OffsetInt(n) => {
                let acc = self.get_acc_int();
                let added = self.builder.ins().iadd_imm(acc, (*n as i64) << 1);
                self.set_acc_int(added);
            }
            &BasicBlockInstruction::CCall1(id) => self.c_call(id, Arity::N1)?,
            &BasicBlockInstruction::CCall2(id) => self.c_call(id, Arity::N2)?,
            &BasicBlockInstruction::CCall3(id) => self.c_call(id, Arity::N3)?,
            &BasicBlockInstruction::CCall4(id) => self.c_call(id, Arity::N4)?,
            &BasicBlockInstruction::CCall5(id) => self.c_call(id, Arity::N5)?,
            &BasicBlockInstruction::CCallN { nargs, id } => {
                self.c_call(id, Arity::VarArgs(nargs))?
            }
            // BasicBlockInstruction::VecTLength => {}
            // BasicBlockInstruction::CheckSignals => {}
            // BasicBlockInstruction::PopTrap => {}
            // BasicBlockInstruction::GetMethod => {}
            // BasicBlockInstruction::SetupForPubMet(_) => {}
            // BasicBlockInstruction::GetDynMet => {}
            _ => bail!("Unimplemented instruction: {:?}", instruction),
        }

        Ok(())
    }

    fn translate_exit(&mut self, exit: &BasicBlockExit) -> Result<()> {
        match exit {
            // BasicBlockExit::Branch(_) => {}
            BasicBlockExit::BranchIf {
                then_block,
                else_block,
            } => {
                let acc_int = self.get_acc_int();
                let cond = self.builder.ins().icmp_imm(IntCC::Equal, acc_int, 1);
                self.builder.ins().brz(cond, self.blocks[*then_block], &[]);
                self.builder.ins().jump(self.blocks[*else_block], &[]);
            }
            BasicBlockExit::BranchCmp {
                cmp,
                constant,
                then_block,
                else_block,
            } => {
                let a = self
                    .builder
                    .ins()
                    .iconst(I64, i64_to_value(*constant as i64));
                let b = self.get_acc_int();
                let cc = comp_to_cc(cmp);
                self.builder
                    .ins()
                    .br_icmp(cc, a, b, self.blocks[*then_block], &[]);
                self.builder.ins().jump(self.blocks[*else_block], &[]);
            }
            BasicBlockExit::Switch { ints, tags } => {
                if ints.len() > 0 && tags.len() > 0 {
                    // We need to check whether the value is an int or a pointer with a tag

                    let ints_switch_block = self.builder.create_block();
                    let tags_switch_blog = self.builder.create_block();

                    let accu = self.get_acc_int();
                    let lsb = self.builder.ins().band_imm(accu, 1);
                    self.builder.ins().brz(lsb, tags_switch_blog, &[]);
                    self.builder.ins().jump(ints_switch_block, &[]);

                    self.builder.seal_block(ints_switch_block);
                    self.builder.seal_block(tags_switch_blog);

                    self.builder.switch_to_block(ints_switch_block);
                    self.emit_int_switch(ints);

                    self.builder.switch_to_block(tags_switch_blog);
                    self.emit_tag_switch(tags);

                    // Otherwise, we only need to emit one of them
                } else if ints.len() > 0 {
                    self.emit_int_switch(ints);
                } else {
                    ensure!(tags.len() > 0);
                    self.emit_tag_switch(tags);
                }
            }
            // BasicBlockExit::PushTrap { normal, trap } => {}
            BasicBlockExit::Return(to_pop) => {
                self.builder.ins().jump(self.return_block, &[]);
                self.pop(*to_pop)?;
            }
            BasicBlockExit::TailCall { args, to_pop } => {
                let args = *args as usize;
                self.push_last_n_items_for_real(args)?;
                let extra_args = self.builder.ins().iconst(I64, args as i64);
                self.builder.def_var(self.return_extra_args_var, extra_args);
                self.builder.ins().jump(self.return_block, &[]);
                self.pop(*to_pop)?;
            }
            // BasicBlockExit::Raise(_) => {}
            // BasicBlockExit::Stop => {}
            _ => bail!("Unimplemented exit: {:?}", exit),
        }
        Ok(())
    }

    // Take self to consume the builder
    fn finalise(mut self) -> Result<()> {
        self.builder.switch_to_block(self.return_block);
        self.builder.seal_block(self.return_block);

        self.save_extern_sp()?;

        let retval = self.get_acc_ref();
        let ret_extra_args = self.builder.use_var(self.return_extra_args_var);
        self.builder.ins().return_(&[retval, ret_extra_args]);

        self.builder.finalize();

        Ok(())
    }

    // Helpers

    fn value_to_long(&mut self, ival: Value) -> Value {
        self.builder.ins().sshr_imm(ival, 1)
    }

    fn long_to_value(&mut self, lval: Value) -> Value {
        let doubled = self.builder.ins().iadd(lval, lval);
        self.builder.ins().iadd_imm(doubled, 1)
    }

    fn check_div_zero(&mut self, divisor: Value) -> Result<()> {
        let raise_block = self.builder.create_block();
        let noraise_block = self.builder.create_block();

        let res = self.builder.ins().icmp_imm(IntCC::Equal, divisor, 1); // Val_long(0)
        self.builder.ins().brnz(res, raise_block, &[]);
        self.builder.ins().jump(noraise_block, &[]);
        self.builder.seal_block(raise_block);
        self.builder.seal_block(noraise_block);

        self.builder.switch_to_block(raise_block);
        self.call_primitive(CraneliftPrimitiveFunction::CamlRaiseZeroDivide, &[])?;
        self.unreachable();

        self.builder.switch_to_block(noraise_block);
        Ok(())
    }


    fn emit_apply(&mut self, num_args: usize) -> Result<()> {
        let closure = self.get_acc_ref();
        let extra_args_val = self.builder.ins().iconst(I64, (num_args - 1) as i64);

        let mut closure_args = vec![];
        for i in 0..num_args {
            closure_args.push(self.pick_ref(i as u32)?);
        }
        self.pop(num_args as u32)?;

        let one = self.builder.ins().iconst(I64, 1);
        let ret_addr =
            self.get_global_variable(I64, CraneliftPrimitiveValue::CallbackReturnAddr)?;
        closure_args.push(ret_addr);
        closure_args.push(one);
        closure_args.push(one);

        let cur_sp = self.get_sp();
        self.save_extern_sp()?;
        let _new_sp = self.push_to_ocaml_stack(cur_sp, &closure_args)?;
        // We don't save the newsp - this is due to interactions with exception handling
        // and callbacks

        let call = self.call_primitive(
            CraneliftPrimitiveFunction::DoCallback,
            &[closure, extra_args_val],
        )?;
        let result = self.builder.inst_results(call)[0];
        self.set_acc_ref(result);
        self.load_extern_sp()?;
        Ok(())
    }

    fn c_call(&mut self, id: u32, arity: Arity) -> Result<()> {
        let local_callee = if let Some(x) = self.used_c_calls.get(&id) {
            *x
        } else {
            let func_id = self
                .primitives
                .get_or_initialise_c_call(self.module, id, arity)?;
            let local_callee = self
                .module
                .declare_func_in_func(func_id, &mut self.builder.func);
            self.used_c_calls.insert(id, local_callee);
            local_callee
        };

        let mut args = vec![];

        let argcount = match arity {
            Arity::N1 => {
                args.push(self.get_acc_ref());
                1
            }
            Arity::N2 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                self.pop(1)?;
                2
            }
            Arity::N3 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                self.pop(2)?;
                3
            }
            Arity::N4 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                args.push(self.pick_ref(2)?);
                self.pop(3)?;
                4
            }
            Arity::N5 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                args.push(self.pick_ref(2)?);
                args.push(self.pick_ref(3)?);
                self.pop(4)?;
                5
            }
            Arity::VarArgs(n) => {
                unimplemented!("VarArgs c_call");
                n
            }
        };

        if self.options.use_call_traces {
            let old_sp = self.get_sp();
            let new_sp = self.push_to_ocaml_stack(old_sp, &args)?;

            let id_val = self.builder.ins().iconst(I32, id as i64);
            let num_args_val = self.builder.ins().iconst(I64, argcount as i64);
            self.call_primitive(
                CraneliftPrimitiveFunction::EmitCCallTrace,
                &[id_val, new_sp, num_args_val],
            )?;

            // Note we're not saving the new_sp to extern_sp - which is equivalent here to a pop
        }

        self.save_extern_sp()?;
        let call = self.builder.ins().call(local_callee, &args);
        let result = self.builder.inst_results(call)[0];
        self.set_acc_ref(result);
        self.load_extern_sp()?;

        if self.options.use_call_traces {
            self.call_primitive(CraneliftPrimitiveFunction::EmitReturnTrace, &[result])?;
        }


        Ok(())
    }

    fn unreachable(&mut self) {
        self.builder.ins().trap(TrapCode::UnreachableCodeReached);
    }

    // Switches

    fn emit_int_switch(&mut self, ints: &[usize]) {
        let mut switch = Switch::new();

        for (i, block_num) in ints.iter().copied().enumerate() {
            switch.set_entry(i as u128, self.blocks[block_num]);
        }

        let fallback_block = self.builder.create_block();

        // Range reduce and convert to ocaml int
        let accu_int = self.get_acc_int();
        let reduced = self.builder.ins().ireduce(I32, accu_int);
        let key = self.builder.ins().sshr_imm(reduced, 1);

        switch.emit(&mut self.builder, key, fallback_block);

        self.builder.seal_block(fallback_block);
        self.builder.switch_to_block(fallback_block);
        self.unreachable();
    }

    fn emit_tag_switch(&mut self, tags: &[usize]) {
        let mut switch = Switch::new();

        for (tag, block_num) in tags.iter().copied().enumerate() {
            switch.set_entry(tag as u128, self.blocks[block_num]);
        }
        let fallback_block = self.builder.create_block();
        let accu_ref = self.get_acc_ref();
        // Assumes little endian - so value[-8] points to the LSBs of the header
        let tag = self
            .builder
            .ins()
            .load(I8, MemFlags::trusted(), accu_ref, -8);
        switch.emit(&mut self.builder, tag, fallback_block);

        self.builder.seal_block(fallback_block);
        self.builder.switch_to_block(fallback_block);
        self.unreachable();
    }

    // Casting
    fn int_to_ref(&mut self, value: Value) -> Value {
        self.builder.ins().raw_bitcast(R64, value)
    }

    fn ref_to_int(&mut self, value: Value) -> Value {
        self.builder.ins().raw_bitcast(I64, value)
    }

    // Stack operations
    fn push_ref(&mut self, value: Value) -> Result<()> {
        if self.stack_size >= self.stack_vars.len() {
            bail!("Stack overflow");
        }

        self.builder
            .def_var(self.stack_vars[self.stack_size], value);
        self.stack_size += 1;

        Ok(())
    }

    fn push_dummy(&mut self, size: usize) -> Result<()> {
        let zero = self.builder.ins().null(R64);
        for _ in 0..size {
            self.push_ref(zero)?;
        }
        Ok(())
    }

    fn push_int(&mut self, value: Value) -> Result<()> {
        let ref_val = self.int_to_ref(value);
        self.push_ref(ref_val)
    }

    fn pick_ref(&mut self, n: u32) -> Result<Value> {
        let n = n as usize;
        if n >= self.stack_size {
            bail!("Stack underflow on pick");
        }

        Ok(self
            .builder
            .use_var(self.stack_vars[self.stack_size - n - 1]))
    }

    fn pick_int(&mut self, n: u32) -> Result<Value> {
        let ref_val = self.pick_ref(n)?;
        Ok(self.ref_to_int(ref_val))
    }

    fn pop(&mut self, n: u32) -> Result<()> {
        let n = n as usize;
        if n > self.stack_size {
            bail!("Stack underflow on pop");
        }

        self.stack_size -= n;
        Ok(())
    }

    // Modifying accu
    fn set_acc_ref(&mut self, value: Value) {
        self.builder.def_var(self.acc, value);
    }

    fn set_acc_int(&mut self, value: Value) {
        let ref_val = self.int_to_ref(value);
        self.set_acc_ref(ref_val);
    }

    fn get_acc_ref(&mut self) -> Value {
        self.builder.use_var(self.acc)
    }

    fn get_acc_int(&mut self) -> Value {
        let ref_val = self.get_acc_ref();
        self.ref_to_int(ref_val)
    }

    // Mopdifying sp
    fn set_sp(&mut self, value: Value) {
        self.builder.def_var(self.sp, value)
    }

    fn get_sp(&mut self) -> Value {
        self.builder.use_var(self.sp)
    }

    // Interfacing with the OCaml interpreter stack

    fn save_extern_sp(&mut self) -> Result<()> {
        let cur_sp = self.get_sp();
        self.set_caml_state_field(CamlStateField::ExternSp, cur_sp);

        Ok(())
    }

    fn load_extern_sp(&mut self) -> Result<()> {
        let new_sp = self.get_caml_state_field(CamlStateField::ExternSp, I64);
        self.set_sp(new_sp);

        Ok(())
    }

    // Pushes count items form the virtual stack to the real stack
    fn push_last_n_items_for_real(&mut self, count: usize) -> Result<Value> {
        let sp = self.get_sp();
        let count = count as i32;

        let new_sp = self.builder.ins().iadd_imm(sp, -8 * count as i64);

        for i in 0..count {
            let val = self.pick_ref(i as u32)?;
            self.builder
                .ins()
                .store(MemFlags::trusted(), val, new_sp, 8 * i);
        }
        self.set_sp(new_sp);
        Ok(new_sp)
    }


    // Returns new sp (but doesn't save it)
    fn push_to_ocaml_stack(&mut self, sp: Value, values: &[Value]) -> Result<Value> {
        let n = values.len() as i64;
        let new_sp = self.builder.ins().iadd_imm(sp, -8 * n);

        for (i, value) in values.iter().enumerate() {
            self.builder
                .ins()
                .store(MemFlags::trusted(), *value, new_sp, 8 * i as i32);
        }

        Ok(new_sp)
    }

    // Getting primitives
    fn get_global_variable(&mut self, typ: Type, value: CraneliftPrimitiveValue) -> Result<Value> {
        let gv = if let Some(gv) = self.used_values.get(&value) {
            *gv
        } else {
            let data_id = self
                .primitives
                .get_or_initialise_value(self.module, value)?;
            let gv = self
                .module
                .declare_data_in_func(data_id, &mut self.builder.func);
            self.used_values.insert(value, gv);
            gv
        };
        Ok(self.builder.ins().symbol_value(typ, gv))
    }

    fn call_primitive(
        &mut self,
        function: CraneliftPrimitiveFunction,
        args: &[Value],
    ) -> Result<Inst> {
        let func_ref = if let Some(func_ref) = self.used_funcs.get(&function) {
            *func_ref
        } else {
            let func_id = self
                .primitives
                .get_or_initialize_function(self.module, function)?;
            let func_ref = self
                .module
                .declare_func_in_func(func_id, &mut self.builder.func);
            self.used_funcs.insert(function, func_ref);
            func_ref
        };

        Ok(self.builder.ins().call(func_ref, args))
    }

    // OCaml state
    fn get_caml_state_field(&mut self, field: CamlStateField, typ: Type) -> Value {
        self.builder.ins().load(
            typ,
            MemFlags::trusted(),
            self.caml_state_addr,
            field.offset(),
        )
    }

    fn set_caml_state_field(&mut self, field: CamlStateField, value: Value) {
        self.builder.ins().store(
            MemFlags::trusted(),
            value,
            self.caml_state_addr,
            field.offset(),
        );
    }

    // Inlining of stuff

    // The arguments are a list of either IR values corresponding to ML values to write (Some) or
    // None (may be useful for non_heap ones)
    fn alloc(&mut self, contents: &[Option<Value>], tag: u8) -> Result<Value> {
        let wosize = contents.len();
        if wosize <= MAX_YOUNG_WOSIZE {
            // If it fits in the minor heap
            let block = self.alloc_small(wosize, tag)?;

            for (i, x) in contents.iter().cloned().enumerate() {
                if let Some(value) = x {
                    self.builder
                        .ins()
                        .store(MemFlags::trusted(), value, block, i as i32 * 8);
                }
            }

            Ok(block)
        } else {
            let wosize_val = self.builder.ins().iconst(I64, wosize as i64);
            let tag = self.builder.ins().iconst(I8, tag as i64);
            let call =
                self.call_primitive(CraneliftPrimitiveFunction::CamlAllocShr, &[wosize_val, tag])?;

            let block = self.builder.inst_results(call)[0];
            let block_i = self.ref_to_int(block);

            for (i, x) in contents.iter().cloned().enumerate() {
                if let Some(value) = x {
                    let addr = self.builder.ins().iadd_imm(block_i, i as i64 * 8);
                    self.call_primitive(
                        CraneliftPrimitiveFunction::CamlInitialize,
                        &[addr, value],
                    )?;
                }
            }

            Ok(block)
        }
    }

    fn alloc_small(&mut self, wosize: usize, tag: u8) -> Result<Value> {
        // From Memory.h
        // Note in our setup profinfo = 0 and track = CAML_DO_TRACK = 1
        // Alloc_small_origin = CAML_FROM_CAML = 2

        // #define Alloc_small_aux(result, wosize, tag, profinfo, track) do {     \
        //   CAMLassert ((wosize) >= 1);                                          \
        //   CAMLassert ((tag_t) (tag) < 256);                                    \
        //   CAMLassert ((wosize) <= Max_young_wosize);                           \
        debug_assert!(wosize >= 1);
        debug_assert!(wosize <= MAX_YOUNG_WOSIZE);

        //   Caml_state_field(young_ptr) -= Whsize_wosize (wosize);               \
        let whsize = wosize + 1;
        let old_young_ptr = self.get_caml_state_field(CamlStateField::YoungPtr, I64);
        let new_young_ptr = self
            .builder
            .ins()
            .iadd_imm(old_young_ptr, -8 * whsize as i64);
        self.set_caml_state_field(CamlStateField::YoungPtr, new_young_ptr);

        //   if (Caml_state_field(young_ptr) < Caml_state_field(young_limit)) {   \
        let young_limit = self.get_caml_state_field(CamlStateField::YoungLimit, I64);
        let call_block = self.builder.create_block();
        let after_block = self.builder.create_block();
        self.builder.append_block_param(after_block, I64); // block param = young_ptr after

        self.builder.ins().br_icmp(
            IntCC::UnsignedLessThan,
            new_young_ptr,
            young_limit,
            call_block,
            &[],
        );
        self.builder.seal_block(call_block);
        self.builder.ins().jump(after_block, &[new_young_ptr]);

        // {
        self.builder.switch_to_block(call_block);

        //     Setup_for_gc;                                                      \
        //     caml_alloc_small_dispatch((wosize), (track) | Alloc_small_origin,  \
        //                               1, NULL);                                \
        //     Restore_after_gc;                                                  \

        let wosize_val = self.builder.ins().iconst(I64, wosize as i64);
        let flags = self.builder.ins().iconst(I32, 0x11); // CAML_DO_TRACK | CAML_FROM_CAML
        let nallocs = self.builder.ins().iconst(I32, 1);
        let encoded_alloc_lens = self.builder.ins().iconst(I64, 0); // null ptr
        self.call_primitive(
            CraneliftPrimitiveFunction::CamlAllocSmallDispatch,
            &[wosize_val, flags, nallocs, encoded_alloc_lens],
        )?;

        // Young ptr has changed likely
        let newest_young_ptr = self.get_caml_state_field(CamlStateField::YoungPtr, I64);
        self.builder.ins().jump(after_block, &[newest_young_ptr]);

        // }

        // We join them together
        self.builder.seal_block(after_block);
        self.builder.switch_to_block(after_block);
        let header_ptr = self.builder.block_params(after_block)[0];

        //   Hd_hp (Caml_state_field(young_ptr)) =                                \
        //     Make_header_with_profinfo ((wosize), (tag), 0, profinfo);          \
        // This goes to Make_header(wosize, tag, color) as we don't use profinfo
        // here color is 0

        let header = make_header(wosize, tag);
        let header_val = self.builder.ins().iconst(I64, header as i64);
        self.builder
            .ins()
            .store(MemFlags::trusted(), header_val, header_ptr, 0);

        //  (result) = Val_hp (Caml_state_field(young_ptr));                     \
        let result_int = self.builder.ins().iadd_imm(header_ptr, 8);

        Ok(self.int_to_ref(result_int))
    }
}

#[derive(Default)]
struct Primitives {
    values: HashMap<CraneliftPrimitiveValue, DataId>,
    functions: HashMap<CraneliftPrimitiveFunction, FuncId>,
    c_calls: HashMap<u32, CCall>,
}

impl Primitives {
    fn get_or_initialise_value<M: Module>(
        &mut self,
        module: &mut M,
        value: CraneliftPrimitiveValue,
    ) -> Result<DataId> {
        Ok(match self.values.get(&value) {
            Some(data_id) => *data_id,
            None => {
                let name: &str = value.into();
                let data_id = module.declare_data(name, Linkage::Import, true, false)?;
                self.values.insert(value, data_id);
                data_id
            }
        })
    }

    fn get_or_initialize_function<M: Module>(
        &mut self,
        module: &mut M,
        function: CraneliftPrimitiveFunction,
    ) -> Result<FuncId> {
        Ok(match self.functions.get(&function) {
            Some(func_id) => *func_id,
            None => {
                let name: &str = function.into();
                let mut signature = module.make_signature();
                create_function_signature(function, &mut signature);

                let func_id = module.declare_function(name, Linkage::Import, &signature)?;
                self.functions.insert(function, func_id);
                func_id
            }
        })
    }

    fn get_or_initialise_c_call<M: Module>(
        &mut self,
        module: &mut M,
        id: u32,
        arity: Arity,
    ) -> Result<FuncId> {
        Ok(match self.c_calls.get(&id) {
            Some(call) => {
                if call.arity != arity {
                    bail!(
                        "Conflicting c-call arities: {:?} first then {:?}",
                        call.arity,
                        arity
                    );
                } else {
                    call.id
                }
            }
            None => {
                let mut sig = module.make_signature();
                sig.returns.push(AbiParam::new(R64));
                match arity {
                    Arity::N1 => {
                        sig.params.push(AbiParam::new(R64));
                    }
                    Arity::N2 => {
                        for _ in 0..2 {
                            sig.params.push(AbiParam::new(R64));
                        }
                    }
                    Arity::N3 => {
                        for _ in 0..3 {
                            sig.params.push(AbiParam::new(R64));
                        }
                    }
                    Arity::N4 => {
                        for _ in 0..4 {
                            sig.params.push(AbiParam::new(R64));
                        }
                    }
                    Arity::N5 => {
                        for _ in 0..5 {
                            sig.params.push(AbiParam::new(R64));
                        }
                    }
                    Arity::VarArgs(_) => {
                        // Pointer to args
                        sig.params.push(AbiParam::new(I64));
                        // Num args
                        sig.params.push(AbiParam::new(I32));
                    }
                }

                let func_id = module.declare_function(
                    &format_c_call_name(id as usize),
                    Linkage::Import,
                    &sig,
                )?;

                self.c_calls.insert(id, CCall { id: func_id, arity });

                func_id
            }
        })
    }
}

fn make_header(wosize: usize, tag: u8) -> i64 {
    let header = ((wosize as u64) << 10) + (tag as u64);
    header as i64
}

fn create_function_signature(function: CraneliftPrimitiveFunction, sig: &mut Signature) {
    match function {
        CraneliftPrimitiveFunction::EmitCCallTrace => {
            sig.params
                .extend(&[AbiParam::new(I32), AbiParam::new(I64), AbiParam::new(I64)]);
        }
        CraneliftPrimitiveFunction::EmitReturnTrace => {
            sig.params.push(AbiParam::new(R64));
        }
        CraneliftPrimitiveFunction::DoCallback => {
            sig.params.extend(&[AbiParam::new(R64), AbiParam::new(I64)]);
            sig.returns
                .extend(&[AbiParam::new(R64), AbiParam::new(I64)]);
        }
        CraneliftPrimitiveFunction::CamlAllocSmallDispatch => {
            sig.params.extend(&[
                AbiParam::new(I64),
                AbiParam::new(I32),
                AbiParam::new(I32),
                AbiParam::new(I64),
            ]);
        }
        CraneliftPrimitiveFunction::MakeBlockTrace => {
            sig.params.extend(&[AbiParam::new(R64)]);
        }
        CraneliftPrimitiveFunction::CamlAllocShr => {
            sig.params.extend(&[AbiParam::new(I64), AbiParam::new(I8)]);
            sig.returns.push(AbiParam::new(R64));
        }
        CraneliftPrimitiveFunction::CamlInitialize => {
            sig.params.extend(&[AbiParam::new(I64), AbiParam::new(R64)]);
        }
        CraneliftPrimitiveFunction::CamlRaiseZeroDivide => {
            // no params/returns
        }
    }
}

fn i64_to_value(i: i64) -> i64 {
    (((i as u64) << 1) as i64) + 1
}

fn comp_to_cc(comp: &Comp) -> IntCC {
    match comp {
        Comp::Eq => IntCC::Equal,
        Comp::Ne => IntCC::NotEqual,
        Comp::Lt => IntCC::SignedLessThan,
        Comp::Le => IntCC::SignedLessThanOrEqual,
        Comp::Gt => IntCC::SignedGreaterThan,
        Comp::Ge => IntCC::SignedGreaterThanOrEqual,
        Comp::ULt => IntCC::UnsignedLessThan,
        Comp::UGe => IntCC::UnsignedGreaterThanOrEqual,
    }
}
