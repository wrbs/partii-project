use crate::interp::GlobalData;
use ocaml_jit_shared::parse_instructions;

pub fn relocate_and_load_instructions(code: &[i32], global_data: &mut GlobalData) {
    let parsed_instructions = parse_instructions(code.iter().copied(), code.len())
        .unwrap_or_else(|e| panic!("Could not parse code: {}", e));

    let output_base = global_data.instructions.len();

    for instruction in parsed_instructions.instructions.iter() {
        let new_instr = instruction.map_labels(|l| {
            let (start_offset, _) = parsed_instructions.lookup[*l]
                .unwrap_or_else(|| panic!("Could not find instruction aligned at {}", l));
            let start = output_base + start_offset;
            start
        });

        global_data.instructions.push(new_instr);
    }

    // Add the lookup table
    global_data
        .lookup
        .add_entry(code, output_base, parsed_instructions.lookup);
}
