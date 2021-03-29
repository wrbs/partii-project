use super::PrintTraces;
use cranelift_jit::{JITBuilder, JITModule};
use ocaml_jit_shared::{
    anyhow::{anyhow, Context, Result},
    basic_blocks::parse_to_basic_blocks,
    cranelift::*,
    cranelift_codegen::settings::{self, Configurable},
    cranelift_compiler::{
        format_c_call_name,
        primitives::{CraneliftPrimitive, CraneliftPrimitiveFunction, CraneliftPrimitiveValue},
        CraneliftCompiler, CraneliftCompilerOptions,
    },
    cranelift_module,
};
use once_cell::unsync::OnceCell;
use std::panic;

use crate::caml::{domain_state::get_extern_sp_addr, misc::CAML_PRIMITIVE_TABLE};

use super::{
    rust_primitives::{emit_c_call_trace, emit_return_trace},
    CompilerData,
};

#[derive(Default)]
pub struct OptimisedCompiler {
    compiler: OnceCell<CraneliftCompiler<JITModule>>,
}

// JITModule isn't send, but the way I use it it's fine (stick it in a mutex)
// the actual problem is raw pointers in the impl of JITModule,
// not anything inherently locked to one thread
unsafe impl Send for OptimisedCompiler {}

impl OptimisedCompiler {
    // Returns absolute address of the compiled closure
    pub fn optimise_closure(
        &mut self,
        section_number: usize,
        code: &[i32],
        entrypoint: usize,
        compiler_data: &mut CompilerData,
    ) -> Result<usize> {
        self.optimise_closure_impl(section_number, code, entrypoint, compiler_data)
            .with_context(|| {
                format!(
                    "Problem compiling closure at section {} offset {}",
                    section_number, entrypoint
                )
            })
    }

    // Uses a separate function to allow wrapping all anyhow errors with the section/offset in the context
    fn optimise_closure_impl(
        &mut self,
        section_number: usize,
        code: &[i32],
        entrypoint: usize,
        compiler_data: &mut CompilerData,
    ) -> Result<usize> {
        self.compiler.get_or_try_init(|| {
            let module = initialise_module(compiler_data);
            CraneliftCompiler::new(module)
        })?;

        let compiler = self.compiler.get_mut().unwrap();
        let func_name = format!("closure_{}_{}", section_number, entrypoint);
        let closure =
            parse_to_basic_blocks(code, entrypoint).context("Problem parsing basic blocks")?;

        let options = CraneliftCompilerOptions {
            use_call_traces: compiler_data.compiler_options.print_traces == Some(PrintTraces::Call),
        };

        // for now replace the hook, so we get better backtraces
        // as cranelift panics a lot
        let old_hook = panic::take_hook();

        let comp_res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            compiler
                .compile_closure(&func_name, &closure, &options, None)
                .context("Problem compiling with cranelift")
        }));
        panic::set_hook(old_hook);
        let func_id = comp_res.map_err(|_| anyhow!("Panic during compilation"))??;

        compiler.module.finalize_definitions();
        let code = compiler.module.get_finalized_function(func_id);
        Ok(code as usize)
    }
}

fn get_isa() -> Box<dyn codegen::isa::TargetIsa> {
    let mut flag_builder = settings::builder();
    flag_builder.set("enable_safepoints", "true").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();
    let isa_builder = cranelift_native::builder().unwrap();
    isa_builder.finish(settings::Flags::new(flag_builder))
}

fn initialise_module(compiler_data: &mut CompilerData) -> JITModule {
    let mut builder = JITBuilder::with_isa(get_isa(), cranelift_module::default_libcall_names());
    define_ocaml_primitives(compiler_data, &mut builder);
    JITModule::new(builder)
}

fn get_prim_value_addr(
    compiler_data: &mut CompilerData,
    primitive: CraneliftPrimitiveValue,
) -> *const u8 {
    match primitive {
        CraneliftPrimitiveValue::OcamlExternSp => get_extern_sp_addr() as _,
        CraneliftPrimitiveValue::CallbackReturnAddr => {
            compiler_data.get_cranelift_apply_return_addr()
        }
    }
}

fn get_prim_function_addr(
    compiler_data: &mut CompilerData,
    primitive: CraneliftPrimitiveFunction,
) -> *const u8 {
    match primitive {
        CraneliftPrimitiveFunction::EmitCCallTrace => emit_c_call_trace as _,
        CraneliftPrimitiveFunction::EmitReturnTrace => emit_return_trace as _,
        CraneliftPrimitiveFunction::DoCallback => compiler_data.get_cranelift_apply_addr(),
    }
}

fn define_ocaml_primitives(compiler_data: &mut CompilerData, builder: &mut JITBuilder) {
    for prim in CraneliftPrimitiveValue::iter() {
        let name: &str = prim.into();
        builder.symbol(name, get_prim_value_addr(compiler_data, prim));
    }

    for prim in CraneliftPrimitiveFunction::iter() {
        let name: &str = prim.into();
        builder.symbol(name, get_prim_function_addr(compiler_data, prim));
    }

    unsafe {
        // Do CCalls
        for (prim_id, defn) in CAML_PRIMITIVE_TABLE.as_slice().iter().enumerate() {
            builder.symbol(&format_c_call_name(prim_id), *defn as *const u8);
        }
    }
}
