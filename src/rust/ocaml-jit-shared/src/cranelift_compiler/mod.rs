use std::{collections::HashMap, fmt::Write};

use crate::basic_blocks::{BasicBlock, BasicBlockExit, BasicBlockInstruction, BasicClosure};
use anyhow::{bail, Context, Result};
use codegen::{
    binemit::{StackMap, StackMapSink},
    ir::GlobalValue,
    print_errors::pretty_error,
};
use cranelift::prelude::*;
use cranelift_module::{DataId, FuncId, Linkage, Module, ModuleError};
use types::{I32, I64, R64};

#[cfg(test)]
mod test;

pub mod primitives;
pub const EXTERN_SP_ADDR_IDENT: &str = "ocaml_extern_sp";

#[derive(Debug, Default)]
pub struct CompilerOutput {
    ir_after_codegen: String,
    ir_after_compile: String,
    disasm: String,
    stack_maps: String,
}

pub struct CraneliftCompiler<M: Module> {
    pub module: M,
    ctx: codegen::Context,
    declared_prims: HashMap<u32, CCall>,
    extern_sp: DataId,
    function_builder_context: FunctionBuilderContext,
}

#[derive(Debug, Default)]
struct StackMaps {
    maps: HashMap<u32, StackMap>,
}

impl StackMapSink for StackMaps {
    fn add_stack_map(&mut self, offset: codegen::binemit::CodeOffset, map: StackMap) {
        self.maps.insert(offset, map);
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

        let extern_sp = module.declare_data(EXTERN_SP_ADDR_IDENT, Linkage::Import, true, false)?;

        Ok(Self {
            module,
            ctx,
            function_builder_context,
            declared_prims: HashMap::new(),
            extern_sp,
        })
    }

    pub fn compile_closure(
        &mut self,
        func_name: &str,
        closure: &BasicClosure,
        mut debug_output: Option<&mut CompilerOutput>,
    ) -> Result<FuncId> {
        self.module.clear_context(&mut self.ctx);

        // Takes one argument - the env
        // Returns two arguments (yes this is possible in System-V calling conv)
        // Ret 1 = return value of closure / what closure to apply if tail-calling
        // Ret 2 = new extra args after function (will turn into a tail call if > 0)
        self.ctx.func.signature.params.push(AbiParam::new(R64));

        // Returns one OCaml value and one extra args
        self.ctx.func.signature.returns.push(AbiParam::new(R64));
        self.ctx.func.signature.returns.push(AbiParam::new(I64));

        let func_id =
            self.module
                .declare_function(func_name, Linkage::Export, &self.ctx.func.signature)?;

        // Compile the function
        // TODO - share this once I stop having errors that stop it from being automatically cleared
        self.function_builder_context = FunctionBuilderContext::new();
        let mut translator = self.create_translator(closure);
        for basic_block in &closure.blocks {
            translator.translate_block(basic_block).with_context(|| {
                format!("Problem compiling basic block {}", basic_block.block_id)
            })?;
        }
        translator.finalise();

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
        let mut stack_map_sink = StackMaps::default();
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
            co.stack_maps.clear();
            co.disasm.clear();

            write!(
                co.ir_after_compile,
                "{}",
                self.ctx.func.display(self.module.isa())
            )
            .unwrap();
            for (offset, map) in &stack_map_sink.maps {
                writeln!(co.stack_maps, "0x{:x}: {:#?}", offset, map).unwrap();
            }

            if let Some(disasm) = self
                .ctx
                .mach_compile_result
                .as_ref()
                .and_then(|d| d.disasm.as_ref())
            {
                write!(co.disasm, "{}", disasm).unwrap();
            }
        }


        Ok(func_id)
    }

    fn create_translator<'a>(&'a mut self, closure: &'a BasicClosure) -> FunctionTranslator<'a, M> {
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
        let stack_vars: Vec<_> = (0..closure.max_stack_size).map(|_| var(R64)).collect();
        let return_var = var(R64);
        let return_extra_args_var = var(I64);

        // Declare the variables
        let env = builder.block_params(blocks[0])[0];

        let extern_sp_glob = self
            .module
            .declare_data_in_func(self.extern_sp, &mut builder.func);
        let extern_sp_addr = builder.ins().symbol_value(I64, extern_sp_glob);

        let cur_sp = builder
            .ins()
            .load(I64, MemFlags::trusted(), extern_sp_addr, 0);

        for i in 0..closure.arity {
            let arg = builder
                .ins()
                .load(R64, MemFlags::trusted(), cur_sp, 8 * i as i32);
            builder.def_var(stack_vars[closure.arity - i - 1], arg);
        }
        let new_sp = builder.ins().iadd_imm(cur_sp, 8 * closure.arity as i64);
        builder
            .ins()
            .store(MemFlags::trusted(), new_sp, extern_sp_addr, 0);
        let stack_size = closure.arity;

        // Zero-initialise the other vars
        let zero = builder.ins().null(R64);
        let zero_i = builder.ins().iconst(I64, 0);
        builder.def_var(acc, zero);
        for i in closure.arity..(closure.max_stack_size as usize) {
            builder.def_var(stack_vars[i], zero);
        }
        builder.def_var(return_var, zero);
        builder.def_var(return_extra_args_var, zero_i);

        FunctionTranslator {
            builder,
            module: &mut self.module,
            stack_size,
            stack_vars,
            env,
            acc,
            blocks,
            declared_prims: &mut self.declared_prims,
            extern_sp_addr,
            return_var,
            return_extra_args_var,
            return_block,
        }
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

struct FunctionTranslator<'a, M>
where
    M: Module,
{
    builder: FunctionBuilder<'a>,
    module: &'a mut M,
    declared_prims: &'a mut HashMap<u32, CCall>,
    stack_size: usize,
    stack_vars: Vec<Variable>,
    extern_sp_addr: Value,
    env: Value,
    acc: Variable,
    return_var: Variable,
    return_extra_args_var: Variable,
    // Represents the blocks in my basic block translation
    blocks: Vec<Block>,
    return_block: Block,
}

impl<'a, M> FunctionTranslator<'a, M>
where
    M: Module,
{
    fn translate_block(&mut self, basic_block: &BasicBlock) -> Result<()> {
        // Start the block
        if basic_block.block_id != 0 {
            self.builder
                .switch_to_block(self.blocks[basic_block.block_id]);
        }
        self.stack_size = basic_block.start_stack_size as usize;

        for instr in &basic_block.instructions {
            self.translate_instruction(instr)?;
        }

        self.translate_exit(&basic_block.exit)?;

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
            // BasicBlockInstruction::EnvAcc(_) => {}
            BasicBlockInstruction::Push => {
                let v = self.get_acc_ref();
                self.push_ref(v)?;
            }
            // BasicBlockInstruction::Pop(_) => {}
            // BasicBlockInstruction::Assign(_) => {}
            // BasicBlockInstruction::Apply1 => {}
            // BasicBlockInstruction::Apply2 => {}
            // BasicBlockInstruction::Apply3 => {}
            // BasicBlockInstruction::PushRetAddr => {}
            // BasicBlockInstruction::Apply(_) => {}
            // BasicBlockInstruction::Closure(_, _) => {}
            // BasicBlockInstruction::ClosureRec(_, _) => {}
            // BasicBlockInstruction::MakeBlock(_, _) => {}
            // BasicBlockInstruction::MakeFloatBlock(_) => {}
            // BasicBlockInstruction::OffsetClosure(_) => {}
            // BasicBlockInstruction::GetGlobal(_) => {}
            // BasicBlockInstruction::SetGlobal(_) => {}
            // BasicBlockInstruction::GetField(_) => {}
            // BasicBlockInstruction::SetField(_) => {}
            // BasicBlockInstruction::GetFloatField(_) => {}
            // BasicBlockInstruction::SetFloatField(_) => {}
            // BasicBlockInstruction::GetVecTItem => {}
            // BasicBlockInstruction::SetVecTItem => {}
            // BasicBlockInstruction::GetBytesChar => {}
            // BasicBlockInstruction::SetBytesChar => {}
            // BasicBlockInstruction::OffsetRef(_) => {}
            // BasicBlockInstruction::Const(_) => {}
            // BasicBlockInstruction::BoolNot => {}
            // BasicBlockInstruction::NegInt => {}
            // BasicBlockInstruction::ArithInt(_) => {}
            // BasicBlockInstruction::IsInt => {}
            // BasicBlockInstruction::IntCmp(_) => {}
            // BasicBlockInstruction::OffsetInt(_) => {}
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
            // BasicBlockExit::BranchCmp {
            //     cmp,
            //     constant,
            //     then_block,
            //     else_block,
            // } => {}
            // BasicBlockExit::Switch { ints, tags } => {}
            // BasicBlockExit::PushTrap { normal, trap } => {}
            BasicBlockExit::Return(to_pop) => {
                let acc = self.get_acc_ref();
                self.builder.def_var(self.return_var, acc);
                self.builder.ins().jump(self.return_block, &[]);
            }
            // BasicBlockExit::TailCall { args, to_pop } => {}
            // BasicBlockExit::Raise(_) => {}
            // BasicBlockExit::Stop => {}
            _ => bail!("Unimplemented exit: {:?}", exit),
        }
        Ok(())
    }

    // Take self to consume the builder
    fn finalise(mut self) {
        self.builder.switch_to_block(self.return_block);
        self.builder.seal_block(self.return_block);
        let retval = self.builder.use_var(self.return_var);
        let ret_extra_args = self.builder.use_var(self.return_extra_args_var);
        self.builder.ins().return_(&[retval, ret_extra_args]);
        self.builder.finalize();
    }

    // Helpers

    fn c_call(&mut self, id: u32, arity: Arity) -> Result<()> {
        let func_id = match self.declared_prims.get(&id) {
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
                let mut sig = self.module.make_signature();
                sig.returns.push(AbiParam::new(R64));
                match arity {
                    Arity::N1 => {}
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

                self.module.declare_function(
                    &format_c_call_name(id as usize),
                    Linkage::Import,
                    &sig,
                )?
            }
        };
        let local_callee = self
            .module
            .declare_func_in_func(func_id, &mut self.builder.func);

        let mut args = vec![];

        match arity {
            Arity::N1 => {
                args.push(self.get_acc_ref());
            }
            Arity::N2 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                self.pop(1)?;
            }
            Arity::N3 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                self.pop(2)?;
            }
            Arity::N4 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                args.push(self.pick_ref(2)?);
                self.pop(3)?;
            }
            Arity::N5 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                args.push(self.pick_ref(2)?);
                args.push(self.pick_ref(3)?);
                self.pop(4)?;
            }
            Arity::VarArgs(_) => {
                unimplemented!("VarArgs c_call")
            }
        };

        let call = self.builder.ins().call(local_callee, &args);
        let result = self.builder.inst_results(call)[0];
        self.set_acc_ref(result);

        Ok(())
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

    fn push_int(&mut self, value: Value) -> Result<()> {
        let ref_val = self.builder.ins().raw_bitcast(R64, value);
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
        Ok(self.builder.ins().raw_bitcast(I64, ref_val))
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
        let ref_val = self.builder.ins().raw_bitcast(R64, value);
        self.set_acc_ref(ref_val);
    }

    fn get_acc_ref(&mut self) -> Value {
        self.builder.use_var(self.acc)
    }

    fn get_acc_int(&mut self) -> Value {
        let ref_val = self.get_acc_ref();
        self.builder.ins().raw_bitcast(I64, ref_val)
    }
}
