use cranelift_jit::{JITBuilder, JITModule};
use ocaml_jit_shared::{cranelift::prelude::*, cranelift_module, cranelift_module::Module};

pub struct OptimisedCompiler {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl Default for OptimisedCompiler {
    fn default() -> Self {
        let builder = JITBuilder::new(cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        }
    }
}

impl OptimisedCompiler {}
