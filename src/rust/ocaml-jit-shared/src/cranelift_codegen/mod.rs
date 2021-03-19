use std::{collections::HashMap, fmt::Write};

use crate::basic_blocks::{
    BasicBlock, BasicBlockExit, BasicBlockInstruction, BasicBlockType, BasicClosure,
};
use anyhow::{bail, Result};
use codegen::{print_errors::pretty_error, Context};
use cranelift::prelude::*;
use cranelift_module::{FuncId, Linkage, Module, ModuleError};

#[cfg(test)]
mod test;

#[derive(Debug, Default)]
pub struct CompilerOutput {
    ir_after_codegen: String,
    ir_after_compile: String,
    disasm: String,
}

pub fn compile_closure<M: Module>(
    func_name: &str,
    closure: &BasicClosure,
    module: &mut M,
    ctx: &mut Context,
    mut debug_output: Option<&mut CompilerOutput>,
) -> Result<()> {
    let mut builder_context = FunctionBuilderContext::new();

    // Takes one argument for the env, then as many arguments as it has OCaml ones
    ctx.func.signature.params.push(AbiParam::new(types::R64));

    for _ in 0..closure.arity {
        ctx.func.signature.params.push(AbiParam::new(types::R64));
    }

    // Returns one OCaml value
    ctx.func.signature.returns.push(AbiParam::new(types::R64));

    let func_id = module.declare_function(func_name, Linkage::Export, &ctx.func.signature)?;

    // Compile the function
    let mut translator = FunctionTranslator::create(closure, module, ctx, &mut builder_context);
    for basic_block in &closure.blocks {
        translator.translate_block(basic_block)?;
    }
    translator.finalise();

    if let Some(co) = debug_output.as_deref_mut() {
        co.ir_after_codegen.clear();
        write!(co.ir_after_codegen, "{}", ctx.func.display(module.isa())).unwrap();
    }

    // Finalise and compile
    ctx.want_disasm = debug_output.is_some();
    match module.define_function(func_id, ctx, &mut codegen::binemit::NullTrapSink {}) {
        Ok(_) => (),
        Err(ModuleError::Compilation(e)) => {
            bail!("{}", pretty_error(&ctx.func, Some(module.isa()), e))
        }

        Err(e) => return Err(e.into()),
    };

    if let Some(co) = debug_output {
        co.ir_after_compile.clear();
        co.disasm.clear();

        write!(co.ir_after_compile, "{}", ctx.func.display(module.isa())).unwrap();

        if let Some(disasm) = ctx
            .mach_compile_result
            .as_ref()
            .and_then(|d| d.disasm.as_ref())
        {
            write!(co.disasm, "{}", disasm).unwrap();
        }
    }

    Ok(())
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
    env: Value,
    acc: Variable,
    return_var: Variable,
    // Represents the blocks in my basic block translation
    blocks: Vec<Block>,
    return_block: Block,
    // Represents the single return exit
    declared_prims: HashMap<u32, CCall>,
}

impl<'a, M> FunctionTranslator<'a, M>
where
    M: Module,
{
    fn create(
        closure: &'a BasicClosure,
        module: &'a mut M,
        ctx: &'a mut Context,
        builder_context: &'a mut FunctionBuilderContext,
    ) -> FunctionTranslator<'a, M> {
        let mut builder = FunctionBuilder::new(&mut ctx.func, builder_context);
        let blocks: Vec<_> = (0..closure.blocks.len())
            .map(|_| builder.create_block())
            .collect();
        let return_block = builder.create_block();
        let entry_block = blocks[0];
        builder.append_block_params_for_function_params(entry_block);
        builder.seal_block(entry_block);
        builder.switch_to_block(entry_block);

        let mut var_count = 0;
        let mut var = || {
            let var = Variable::new(var_count);
            var_count += 1;
            builder.declare_var(var, types::R64);
            var
        };

        let acc = var();
        let stack_vars: Vec<_> = (0..closure.max_stack_size).map(|_| var()).collect();
        let return_var = var();

        // Declare the variables
        let env = builder.block_params(blocks[0])[0];

        dbg!(&stack_vars);
        dbg!(builder.block_params(blocks[0]));
        for i in 0..closure.arity {
            dbg!(i);
            builder.def_var(stack_vars[i], builder.block_params(blocks[0])[i + 1])
        }
        let stack_size = closure.arity;

        // Zero-initialise the other vars
        let zero = builder.ins().null(types::R64);
        builder.def_var(acc, zero);
        for i in closure.arity..(closure.max_stack_size as usize) {
            builder.def_var(stack_vars[i], zero);
        }
        builder.def_var(return_var, zero);

        FunctionTranslator {
            builder,
            module,
            stack_size,
            stack_vars,
            env,
            acc,
            blocks,
            declared_prims: HashMap::new(),
            return_var,
            return_block,
        }
    }

    fn translate_block(&mut self, basic_block: &BasicBlock) -> Result<()> {
        dbg!(&basic_block);
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
            BasicBlockInstruction::EnvAcc(_) => {}
            BasicBlockInstruction::Push => {
                let v = self.get_acc_ref();
                self.push_ref(v)?;
            }
            BasicBlockInstruction::Pop(_) => {}
            BasicBlockInstruction::Assign(_) => {}
            BasicBlockInstruction::Apply1 => {}
            BasicBlockInstruction::Apply2 => {}
            BasicBlockInstruction::Apply3 => {}
            BasicBlockInstruction::PushRetAddr => {}
            BasicBlockInstruction::Apply(_) => {}
            BasicBlockInstruction::Closure(_, _) => {}
            BasicBlockInstruction::ClosureRec(_, _) => {}
            BasicBlockInstruction::MakeBlock(_, _) => {}
            BasicBlockInstruction::MakeFloatBlock(_) => {}
            BasicBlockInstruction::OffsetClosure(_) => {}
            BasicBlockInstruction::GetGlobal(_) => {}
            BasicBlockInstruction::SetGlobal(_) => {}
            BasicBlockInstruction::GetField(_) => {}
            BasicBlockInstruction::SetField(_) => {}
            BasicBlockInstruction::GetFloatField(_) => {}
            BasicBlockInstruction::SetFloatField(_) => {}
            BasicBlockInstruction::GetVecTItem => {}
            BasicBlockInstruction::SetVecTItem => {}
            BasicBlockInstruction::GetBytesChar => {}
            BasicBlockInstruction::SetBytesChar => {}
            BasicBlockInstruction::OffsetRef(_) => {}
            BasicBlockInstruction::Const(_) => {}
            BasicBlockInstruction::BoolNot => {}
            BasicBlockInstruction::NegInt => {}
            BasicBlockInstruction::ArithInt(_) => {}
            BasicBlockInstruction::IsInt => {}
            BasicBlockInstruction::IntCmp(_) => {}
            BasicBlockInstruction::OffsetInt(_) => {}
            &BasicBlockInstruction::CCall1(id) => self.c_call(id, Arity::N1)?,
            &BasicBlockInstruction::CCall2(id) => self.c_call(id, Arity::N2)?,
            &BasicBlockInstruction::CCall3(id) => self.c_call(id, Arity::N3)?,
            &BasicBlockInstruction::CCall4(id) => self.c_call(id, Arity::N4)?,
            &BasicBlockInstruction::CCall5(id) => self.c_call(id, Arity::N5)?,
            &BasicBlockInstruction::CCallN { nargs, id } => {
                self.c_call(id, Arity::VarArgs(nargs))?
            }
            BasicBlockInstruction::VecTLength => {}
            BasicBlockInstruction::CheckSignals => {}
            BasicBlockInstruction::PopTrap => {}
            BasicBlockInstruction::GetMethod => {}
            BasicBlockInstruction::SetupForPubMet(_) => {}
            BasicBlockInstruction::GetDynMet => {}
        }

        Ok(())
    }

    fn translate_exit(&mut self, exit: &BasicBlockExit) -> Result<()> {
        match exit {
            BasicBlockExit::Branch(_) => {}
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
            } => {}
            BasicBlockExit::Switch { ints, tags } => {}
            BasicBlockExit::PushTrap { normal, trap } => {}
            BasicBlockExit::Return(to_pop) => {
                let acc = self.get_acc_ref();
                self.builder.def_var(self.return_var, acc);
                self.builder.ins().jump(self.return_block, &[]);
            }
            BasicBlockExit::TailCall { args, to_pop } => {}
            BasicBlockExit::Raise(_) => {}
            BasicBlockExit::Stop => {}
        }
        Ok(())
    }

    // Take self to consume the builder
    fn finalise(mut self) {
        self.builder.switch_to_block(self.return_block);
        self.builder.seal_block(self.return_block);
        let retval = self.builder.use_var(self.return_var);
        self.builder.ins().return_(&[retval]);
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
                sig.returns.push(AbiParam::new(types::R64));
                match arity {
                    Arity::N1 => {}
                    Arity::N2 => {
                        for _ in 0..2 {
                            sig.params.push(AbiParam::new(types::R64));
                        }
                    }
                    Arity::N3 => {
                        for _ in 0..2 {
                            sig.params.push(AbiParam::new(types::R64));
                        }
                    }
                    Arity::N4 => {
                        for _ in 0..2 {
                            sig.params.push(AbiParam::new(types::R64));
                        }
                    }
                    Arity::N5 => {
                        for _ in 0..2 {
                            sig.params.push(AbiParam::new(types::R64));
                        }
                    }
                    Arity::VarArgs(_) => {
                        // Pointer to args
                        sig.params.push(AbiParam::new(types::I64));
                        // Num args
                        sig.params.push(AbiParam::new(types::I32));
                    }
                }

                self.module
                    .declare_function(&format!("ccall{}", id), Linkage::Import, &sig)?
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
                args.push(self.pick_ref(3)?);
                self.pop(3)?;
            }
            Arity::N5 => {
                args.push(self.get_acc_ref());
                args.push(self.pick_ref(0)?);
                args.push(self.pick_ref(1)?);
                args.push(self.pick_ref(3)?);
                args.push(self.pick_ref(4)?);
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

        Ok(())
    }

    fn push_int(&mut self, value: Value) -> Result<()> {
        let ref_val = self.builder.ins().raw_bitcast(types::R64, value);
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
        Ok(self.builder.ins().raw_bitcast(types::I64, ref_val))
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
        let ref_val = self.builder.ins().raw_bitcast(types::R64, value);
        self.set_acc_ref(ref_val);
    }

    fn get_acc_ref(&mut self) -> Value {
        self.builder.use_var(self.acc)
    }

    fn get_acc_int(&mut self) -> Value {
        let ref_val = self.get_acc_ref();
        self.builder.ins().raw_bitcast(types::I64, ref_val)
    }
}
