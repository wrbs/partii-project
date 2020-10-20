mod c_primitives;
mod emit_code;
mod saved_data;

use crate::caml::mlvalues::Value;
use ocaml_jit_shared::{
    get_bytecode_references, parse_instructions_from_code_slice, relocate_instructions,
};
pub use saved_data::{CompilerData, Section};
use std::ops::Deref;
use std::path::Path;

pub fn compile<P: AsRef<Path>>(
    compiler_data: &mut CompilerData,
    code: &[i32],
    print_traces: bool,
    save_compiled_path: Option<P>,
) {
    let parsed_instructions = parse_instructions_from_code_slice(code)
        .unwrap_or_else(|e| panic!("Could not parse code: {}", e));

    let relocated_instructions = relocate_instructions(&parsed_instructions)
        .unwrap_or_else(|| panic!("Could not relocate instructions"));
    let bytecode_offsets = get_bytecode_references(&parsed_instructions);

    let (code_buffer, entrypoint) =
        emit_code::compile_instructions(&relocated_instructions, &bytecode_offsets, print_traces);

    if let Some(p) = save_compiled_path {
        std::fs::write(p, code_buffer.deref()).unwrap();
    }

    compiler_data.sections.push(Section::new(
        code,
        code_buffer,
        entrypoint,
        relocated_instructions,
    ));
}

pub fn get_entrypoint(compiler_data: &CompilerData, code: &[i32]) -> impl Fn() -> Value {
    let entrypoint = compiler_data
        .get_section(code)
        .expect("Section not compiled!")
        .entrypoint;

    move || entrypoint()
}
