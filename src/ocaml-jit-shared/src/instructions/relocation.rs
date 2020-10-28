use super::ParsedInstructions;
use crate::instructions::parse::BytecodeLookupEntry;
use crate::instructions::types::{BytecodeRelativeOffset, ParsedRelativeOffset};
use crate::Instruction;

pub fn relocate_instructions(
    parsed_instructions: &ParsedInstructions,
) -> Option<Vec<Instruction<ParsedRelativeOffset>>> {
    let mut relocated = Vec::with_capacity(parsed_instructions.instructions.len());

    for instruction in parsed_instructions.instructions.iter() {
        let mut ok = true;
        let new_instr = instruction.map_labels(|l| {
            if let Some(BytecodeLookupEntry { start_offset, .. }) =
                parsed_instructions.lookup_bytecode_offset(*l)
            {
                start_offset
            } else {
                ok = false;
                // We can't return None in the parent function from within the closure, so we return
                // a false value and remember to return None later
                ParsedRelativeOffset(0)
            }
        });

        if !ok {
            return None;
        }

        relocated.push(new_instr);
    }

    Some(relocated)
}

pub fn get_bytecode_references(
    parsed_instructions: &ParsedInstructions,
) -> Vec<Option<BytecodeRelativeOffset>> {
    let mut references = vec![None; parsed_instructions.instructions.len()];

    for (bytecode_offset, entry) in parsed_instructions.lookup_data.iter().enumerate() {
        if let Some(BytecodeLookupEntry { start_offset, .. }) = entry {
            references[start_offset.0] = Some(BytecodeRelativeOffset(bytecode_offset))
        }
    }

    references
}
