use crate::interp::GlobalData;
use ocaml_jit_shared::parse_instructions;
use crate::interp::compiler::compile;

pub fn relocate_and_load_instructions(code: &[i32]) {
    let parsed_instructions = parse_instructions(code.iter().copied(), code.len())
        .unwrap_or_else(|e| panic!("Could not parse code: {}", e));

    let mut relocated = Vec::with_capacity(parsed_instructions.instructions.len());

    for instruction in parsed_instructions.instructions.iter() {
        let new_instr = instruction.map_labels(|l| {
            let (start_offset, _) = parsed_instructions.lookup[*l]
                .unwrap_or_else(|| panic!("Could not find instruction aligned at {}", l));
            let start = start_offset;
            start
        });

        relocated.push(new_instr);
    }

    compile(&relocated);
}
