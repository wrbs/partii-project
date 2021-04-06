use super::{
    c_primitives::{
        caml_alloc_shr, caml_alloc_small_dispatch, caml_initialize, caml_modify,
        caml_process_pending_actions, caml_raise, caml_raise_zero_divide, get_global_data_addr,
        get_something_to_do_addr, jit_support_get_dyn_met, jit_support_vect_length,
    },
    rust_primitives::{emit_enter_apply_trace, make_block_trace},
    PrintTraces,
};
use cranelift_jit::{JITBuilder, JITModule};
use ocaml_jit_shared::{
    anyhow::{anyhow, Context, Result},
    basic_blocks::parse_to_basic_blocks,
    cranelift::*,
    cranelift_codegen::{
        binemit::StackMap,
        settings::{self, Configurable},
    },
    cranelift_compiler::{
        format_c_call_name,
        primitives::{CraneliftPrimitive, CraneliftPrimitiveFunction, CraneliftPrimitiveValue},
        CompilationResult, CraneliftCompiler, CraneliftCompilerOptions,
    },
    cranelift_module,
};
use once_cell::unsync::OnceCell;
use std::{collections::HashMap, panic};

use crate::caml::{
    domain_state::get_caml_state_addr, misc::CAML_PRIMITIVE_TABLE, mlvalues::get_atom_table_addr,
};

use super::{
    rust_primitives::{emit_c_call_trace, emit_return_trace},
    CompilerData,
};

#[derive(Default)]
pub struct OptimisedCompiler {
    compiler: OnceCell<CraneliftCompiler<JITModule>>,
    stack_maps: HashMap<u64, StackMap>,
    stack_maps_todo: Vec<(u32, StackMap)>,
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
    ) -> Result<Option<usize>> {
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
    ) -> Result<Option<usize>> {
        self.compiler.get_or_try_init(|| {
            let module = initialise_module(compiler_data);
            let atom_table_addr = get_atom_table_addr();
            CraneliftCompiler::new(module, atom_table_addr)
        })?;

        self.stack_maps_todo.clear();

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
        let stack_maps = &mut self.stack_maps_todo;

        let closure_addresses = &compiler_data.sections[section_number]
            .as_ref()
            .unwrap()
            .closure_addresses;
        let lookup_closure_code = |offset| closure_addresses.get(&offset).map(|x| *x as *const u8);

        let comp_res_res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            compiler
                .compile_closure(
                    &func_name,
                    &closure,
                    lookup_closure_code,
                    &options,
                    None,
                    stack_maps,
                )
                .context("Problem compiling with cranelift")
        }));
        panic::set_hook(old_hook);
        let comp_res = comp_res_res.map_err(|_| anyhow!("Panic during compilation"))??;
        let func_id = match comp_res {
            CompilationResult::UnsupportedClosure => return Ok(None),
            CompilationResult::SupportedClosure(func_id) => func_id,
        };

        compiler.module.finalize_definitions();
        let code = compiler.module.get_finalized_function(func_id);

        for (offset, map) in self.stack_maps_todo.drain(..) {
            self.stack_maps.insert(code as u64 + offset as u64, map);
        }
        Ok(Some(code as usize))
    }

    pub fn lookup_stack_map<F: FnMut(usize)>(&self, return_addr: u64, mut f: F) {
        if let Some(map) = self.stack_maps.get(&return_addr) {
            for x in 0..map.mapped_words() {
                let bit = x as usize;
                if map.get_bit(bit) {
                    f(bit + 1);
                }
            }
        }
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
        CraneliftPrimitiveValue::CamlStateAddr => get_caml_state_addr(),
        CraneliftPrimitiveValue::CallbackReturnAddr => {
            compiler_data.get_cranelift_apply_addresses().return_addr as _
        }
        CraneliftPrimitiveValue::GlobalDataAddr => get_global_data_addr() as _,
        CraneliftPrimitiveValue::CamlSomethingToDoAddr => get_something_to_do_addr() as _,
    }
}

fn get_prim_function_addr(
    compiler_data: &mut CompilerData,
    primitive: CraneliftPrimitiveFunction,
) -> *const u8 {
    match primitive {
        CraneliftPrimitiveFunction::EmitApplyTrace => emit_enter_apply_trace as _,
        CraneliftPrimitiveFunction::EmitCCallTrace => emit_c_call_trace as _,
        CraneliftPrimitiveFunction::EmitReturnTrace => emit_return_trace as _,
        CraneliftPrimitiveFunction::Apply1 => {
            compiler_data.get_cranelift_apply_addresses().apply_1 as _
        }
        CraneliftPrimitiveFunction::Apply2 => {
            compiler_data.get_cranelift_apply_addresses().apply_2 as _
        }
        CraneliftPrimitiveFunction::Apply3 => {
            compiler_data.get_cranelift_apply_addresses().apply_3 as _
        }
        CraneliftPrimitiveFunction::Apply4 => {
            compiler_data.get_cranelift_apply_addresses().apply_4 as _
        }
        CraneliftPrimitiveFunction::Apply5 => {
            compiler_data.get_cranelift_apply_addresses().apply_5 as _
        }
        CraneliftPrimitiveFunction::ApplyN => {
            compiler_data.get_cranelift_apply_addresses().apply_n as _
        }
        CraneliftPrimitiveFunction::CamlAllocSmallDispatch => caml_alloc_small_dispatch as _,
        CraneliftPrimitiveFunction::CamlAllocShr => caml_alloc_shr as _,
        CraneliftPrimitiveFunction::CamlInitialize => caml_initialize as _,
        CraneliftPrimitiveFunction::CamlModify => caml_modify as _,
        CraneliftPrimitiveFunction::CamlRaiseZeroDivide => caml_raise_zero_divide as _,

        CraneliftPrimitiveFunction::MakeBlockTrace => make_block_trace as _,
        CraneliftPrimitiveFunction::CamlProcessPendingActions => caml_process_pending_actions as _,
        CraneliftPrimitiveFunction::CamlRaise => caml_raise as _,
        CraneliftPrimitiveFunction::JitSupportVectLength => jit_support_vect_length as _,
        CraneliftPrimitiveFunction::JitSupportGetDynMet => jit_support_get_dyn_met as _,
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
