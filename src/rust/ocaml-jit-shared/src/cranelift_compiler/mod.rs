use std::{collections::HashMap, fmt::Write};

use crate::basic_blocks::{BasicBlock, BasicBlockExit, BasicBlockInstruction, BasicClosure};
use anyhow::{bail, Context, Result};
use codegen::{
    binemit::{StackMap, StackMapSink},
    ir::{FuncRef, GlobalValue},
    print_errors::pretty_error,
};
use cranelift::prelude::*;
use cranelift_module::{DataId, FuncId, Linkage, Module, ModuleError};
use types::{I32, I64, R64};

use self::primitives::{CraneliftPrimitiveFunction, CraneliftPrimitiveValue};

#[cfg(test)]
mod test;

pub mod primitives;

#[derive(Debug, Default)]
pub struct CompilerOutput {
    ir_after_codegen: String,
    ir_after_compile: String,
    disasm: String,
    stack_maps: String,
}

pub struct CraneliftCompilerOptions {
    pub use_call_traces: bool,
}

pub struct CraneliftCompiler<M: Module> {
    pub module: M,
    ctx: codegen::Context,
    declared_prims: HashMap<u32, CCall>,
    function_builder_context: FunctionBuilderContext,
    primitives: Primitives,
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


        Ok(Self {
            module,
            ctx,
            function_builder_context,
            declared_prims: HashMap::new(),
            primitives: Primitives::default(),
        })
    }

    pub fn compile_closure(
        &mut self,
        func_name: &str,
        closure: &BasicClosure,
        options: &CraneliftCompilerOptions,
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
        let mut translator = self.create_translator(closure, options)?;
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

    fn create_translator<'a>(
        &'a mut self,
        closure: &'a BasicClosure,
        options: &'a CraneliftCompilerOptions,
    ) -> Result<FunctionTranslator<'a, M>> {
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
        let stack_size = closure.arity;
        let env = builder.block_params(blocks[0])[0];

        let mut ft = FunctionTranslator {
            builder,
            module: &mut self.module,
            stack_size,
            stack_vars,
            env,
            acc,
            blocks,
            options,
            return_var,
            return_extra_args_var,
            return_block,
            primitives: &mut self.primitives,
            used_c_calls: HashMap::new(),
            used_funcs: HashMap::new(),
            used_values: HashMap::new(),
        };

        // Declare the variables
        let extern_sp_addr = ft.get_global_variable(I64, CraneliftPrimitiveValue::OcamlExternSp)?;

        let cur_sp = ft
            .builder
            .ins()
            .load(I64, MemFlags::trusted(), extern_sp_addr, 0);

        for i in 0..closure.arity {
            let arg = ft
                .builder
                .ins()
                .load(R64, MemFlags::trusted(), cur_sp, 8 * i as i32);
            ft.builder
                .def_var(ft.stack_vars[closure.arity - i - 1], arg);
        }
        let new_sp = ft.builder.ins().iadd_imm(cur_sp, 8 * closure.arity as i64);
        ft.builder
            .ins()
            .store(MemFlags::trusted(), new_sp, extern_sp_addr, 0);

        // Zero-initialise the other vars
        let zero = ft.builder.ins().null(R64);
        let zero_i = ft.builder.ins().iconst(I64, 0);
        ft.builder.def_var(acc, zero);
        for i in closure.arity..(closure.max_stack_size as usize) {
            ft.builder.def_var(ft.stack_vars[i], zero);
        }
        ft.builder.def_var(return_var, zero);
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

struct FunctionTranslator<'a, M>
where
    M: Module,
{
    builder: FunctionBuilder<'a>,
    module: &'a mut M,
    stack_size: usize,
    stack_vars: Vec<Variable>,
    options: &'a CraneliftCompilerOptions,
    env: Value,
    acc: Variable,
    return_var: Variable,
    return_extra_args_var: Variable,
    // Represents the blocks in my basic block translation
    blocks: Vec<Block>,
    return_block: Block,
    // Primitives
    primitives: &'a mut Primitives,
    used_values: HashMap<CraneliftPrimitiveValue, GlobalValue>,
    used_funcs: HashMap<CraneliftPrimitiveFunction, FuncRef>,
    used_c_calls: HashMap<u32, FuncRef>,
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
}

#[derive(Default)]
struct Primitives {
    values: HashMap<CraneliftPrimitiveValue, DataId>,
    functions: HashMap<CraneliftPrimitiveFunction, (FuncId, Signature)>,
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
    ) -> Result<(FuncId, Signature)> {
        Ok(match self.functions.get(&function) {
            Some((func_id, signature)) => (*func_id, signature.clone()),
            None => {
                let name: &str = function.into();
                let mut signature = module.make_signature();
                create_function_signature(function, &mut signature);

                let func_id = module.declare_function(name, Linkage::Import, &signature)?;
                let clone = signature.clone();
                self.functions.insert(function, (func_id, signature));
                (func_id, clone)
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

fn create_function_signature(function: CraneliftPrimitiveFunction, sig: &mut Signature) {
    match function {}
}
