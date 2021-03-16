use anyhow::{bail, Result};

use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    values::{IntValue, PointerValue},
};
use ocaml_jit_shared::Instruction;

use super::{data::Closure, ssa::get_closure_info};

struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    stack: Vec<PointerValue<'ctx>>,
    current_stack_size: usize,
}

fn emit_llvm_ir(closure: &Closure) -> Result<()> {
    let context = Context::create();
    let module = context.create_module("closure");
    let builder = context.create_builder();

    let (order, stack_star, max_stack_size) = get_closure_info(closure)?;

    let i64_type = context.i64_type();
    let fn_type = i64_type.fn_type(&[], false);
    let function = module.add_function("closure", fn_type, None);
    let basic_block = context.append_basic_block(function, "entry");

    let mut stack = Vec::new();

    for i in 0..max_stack_size {
        stack.push(builder.build_alloca(i64_type, &format!("stack{}", i)));
    }

    let current_stack_size = 0;

    let mut codegen = CodeGen {
        context: &context,
        module,
        builder,
        stack,
        current_stack_size,
    };

    codegen.compile_closure(closure)
}

impl<'ctx> CodeGen<'ctx> {
    fn peek(&self, n: usize) -> Result<IntValue<'ctx>> {
        if n >= self.current_stack_size {
            bail!("Peek past start of stack!");
        }

        let alloca_value = self.stack[self.current_stack_size - n - 1];
        let int_value = self
            .builder
            .build_load(alloca_value, "peek")
            .into_int_value();

        Ok(int_value)
    }

    fn push_frame(&mut self, n: usize) -> Result<()> {
        if self.current_stack_size + n > self.stack.len() {
            bail!("Push_frame past end of stack!")
        }

        self.current_stack_size += n;

        Ok(())
    }

    fn push(&mut self, val: IntValue<'ctx>) -> Result<()> {
        if self.current_stack_size == self.stack.len() {
            bail!("Push past end of stack!")
        }

        let alloca_value = self.stack[self.current_stack_size];
        self.builder.build_store(alloca_value, val);

        Ok(())
    }

    fn pop(&mut self, n: usize) -> Result<()> {
        if n > self.current_stack_size {
            bail!("Pop past end of stack!");
        }
        self.current_stack_size -= n;
        Ok(())
    }

    fn compile_closure(&mut self, closure: &Closure) -> Result<()> {
        Ok(())
    }
}

enum BlockIterResult<'a> {
    Instruction(&'a Instruction<usize>, isize),
    End(),
}
