mod c_primitives;
mod emit_code;
mod saved_data;

pub use saved_data::{CompilerData, EntryPoint, LongjmpEntryPoint, LongjmpHandler, Section};

pub fn compile(compiler_data: &mut CompilerData, bytecode: &[i32], print_traces: bool) -> usize {
    let section_number = compiler_data.sections.len();

    let (compiled_code, entrypoint, first_instr, parsed_instructions) =
        emit_code::compile_instructions(section_number, bytecode, print_traces);

    compiler_data.sections.push(Some(Section::new(
        section_number,
        bytecode,
        compiled_code,
        entrypoint,
        parsed_instructions,
        first_instr as usize,
    )));

    section_number
}

pub fn get_entrypoint(compiler_data: &CompilerData, code: &[i32]) -> EntryPoint {
    compiler_data
        .get_section_for_code(code)
        .expect("Section not compiled!")
        .entrypoint
}

pub fn compile_callback_if_needed(
    compiler_data: &mut CompilerData,
    code: &[i32],
    print_traces: bool,
) -> Option<usize> {
    if compiler_data.callback_compiled {
        return None;
    }

    let section_number = compiler_data.sections.len();
    let (compiled_code, entrypoint, first_instr) =
        emit_code::emit_callback_entrypoint(section_number, print_traces, code);

    compiler_data.sections.push(Some(Section::new(
        section_number,
        code,
        compiled_code,
        entrypoint,
        None,
        first_instr as usize,
    )));

    compiler_data.callback_compiled = true;

    Some(section_number)
}
