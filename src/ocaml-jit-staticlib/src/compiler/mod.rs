mod c_primitives;
mod emit_code;
mod saved_data;

use ocaml_jit_shared::parse_instructions_from_code_slice;
pub use saved_data::{CompilerData, EntryPoint, LongjmpEntryPoint, LongjmpHandler, Section};
use std::path::Path;

pub fn compile<P: AsRef<Path>>(
    compiler_data: &mut CompilerData,
    bytecode: &[i32],
    print_traces: bool,
    save_compiled_path: Option<P>,
) {
    let section_number = compiler_data.sections.len();

    let parsed_instructions = parse_instructions_from_code_slice(bytecode)
        .unwrap_or_else(|e| panic!("Could not parse code: {}", e));

    let (compiled_code, entrypoint) = emit_code::compile_instructions(
        section_number,
        &parsed_instructions.instructions,
        bytecode,
        print_traces,
    );

    if let Some(p) = save_compiled_path {
        std::fs::write(p, &*compiled_code).unwrap();
    }

    compiler_data.sections.push(Some(Section::new(
        section_number,
        bytecode,
        compiled_code,
        entrypoint,
        parsed_instructions.instructions,
    )));
}

pub fn get_entrypoint(compiler_data: &CompilerData, code: &[i32]) -> EntryPoint {
    compiler_data
        .get_section_for_code(code)
        .expect("Section not compiled!")
        .entrypoint
}
