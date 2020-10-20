use super::ParsedInstructions;
use crate::Instruction;

pub fn relocate_instructions(
    parsed_instructions: &ParsedInstructions,
) -> Option<Vec<Instruction<usize>>> {
    let mut relocated = Vec::with_capacity(parsed_instructions.instructions.len());

    for instruction in parsed_instructions.instructions.iter() {
        let mut ok = true;
        let new_instr = instruction.map_labels(|l| {
            if let Some(Some((start_offset, _))) = parsed_instructions.lookup.get(*l) {
                *start_offset
            } else {
                ok = false;
                // We can't return None in the parent function from within the closure, so we return
                // a false value and remember to return None later
                0
            }
        });

        if !ok {
            return None;
        }

        relocated.push(new_instr);
    }

    Some(relocated)
}

pub fn get_bytecode_references(parsed_instructions: &ParsedInstructions) -> Vec<Option<usize>> {
    let mut references = vec![None; parsed_instructions.instructions.len()];

    for (bytecode_offset, entry) in parsed_instructions.lookup.iter().enumerate() {
        if let Some((parsed_offset, _)) = entry {
            references[*parsed_offset] = Some(bytecode_offset)
        }
    }

    references
}
