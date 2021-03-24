use cranelift_jit::{JITBuilder, JITModule};
use ocaml_jit_shared::{
    anyhow::{anyhow, Context, Result},
    basic_blocks::parse_to_basic_blocks,
    cranelift::*,
    cranelift_codegen::settings::{self, Configurable},
    cranelift_compiler::{format_c_call_name, CraneliftCompiler, EXTERN_SP_ADDR_IDENT},
    cranelift_module,
};
use once_cell::unsync::OnceCell;
use std::panic;

use crate::caml::{domain_state::get_extern_sp_addr, misc::CAML_PRIMITIVE_TABLE};

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
    ) -> Result<usize> {
        self.optimise_closure_impl(section_number, code, entrypoint)
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
    ) -> Result<usize> {
        self.compiler.get_or_try_init(|| {
            let module = initialise_module();
            CraneliftCompiler::new(module)
        })?;

        let compiler = self.compiler.get_mut().unwrap();
        let func_name = format!("closure_{}_{}", section_number, entrypoint);
        let closure =
            parse_to_basic_blocks(code, entrypoint).context("Problem parsing basic blocks")?;

        // for now replace the hook, so we get better backtraces
        // as cranelift panics a lot
        let old_hook = panic::take_hook();

        let comp_res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            compiler
                .compile_closure(&func_name, &closure, None)
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

fn initialise_module() -> JITModule {
    let mut builder = JITBuilder::with_isa(get_isa(), cranelift_module::default_libcall_names());
    define_ocaml_primitives(&mut builder);
    JITModule::new(builder)
}

fn define_ocaml_primitives(builder: &mut JITBuilder) {
    builder.symbol(EXTERN_SP_ADDR_IDENT, get_extern_sp_addr() as *const u8);
    unsafe {
        for (prim_id, defn) in CAML_PRIMITIVE_TABLE.as_slice().iter().enumerate() {
            builder.symbol(&format_c_call_name(prim_id), *defn as *const u8);
        }
    }
}
