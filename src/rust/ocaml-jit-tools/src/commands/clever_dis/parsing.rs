use super::data::*;

use crate::bytecode_files::{BytecodeFile, MLValue};
use anyhow::{anyhow, bail, Result};
use ocaml_jit_shared::{BytecodeRelativeOffset, Instruction};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn process_bytecode(bcf: BytecodeFile) -> Result<Program> {
    let mut ctx = {
        let instructions = bcf.parse_instructions()?;

        let mut label_to_index_map = HashMap::new();
        let mut referenced_labels = HashSet::new();

        for (index, instruction) in instructions.iter().enumerate() {
            if let Instruction::LabelDef(l) = instruction {
                label_to_index_map.insert(l.0, index);
            } else {
                let _ = instruction.map_labels(|l| {
                    referenced_labels.insert(l.0);
                });
            }
        }

        let mut closures_todo = VecDeque::new();
        closures_todo.push_back(0);
        let mut closure_nums = HashMap::new();
        closure_nums.insert(0, 0);

        GlobalCtx {
            instructions,
            label_to_index_map,
            referenced_labels,
            closures_todo,
            closure_nums,
            parent_closures: HashMap::new(),
        }
    };

    let mut closures = Vec::new();

    while let Some(entrypoint) = ctx.closures_todo.pop_front() {
        closures.push(process_closure(&mut ctx, entrypoint)?);
    }

    let mut globals: Vec<GlobalTableEntry> = match bcf.global_data {
        MLValue::Block(block_id) => match bcf.global_data_blocks.get_block(block_id) {
            Some((_, items)) => items
                .iter()
                .cloned()
                .map(|v| GlobalTableEntry::Constant(v))
                .collect(),
            _ => bail!("Invalid global data format"),
        },
        _ => bail!("Invalid global data format"),
    };

    for (index, id) in bcf.symbol_table.iter() {
        globals[*index] = GlobalTableEntry::Global(id.clone());
    }

    Ok(Program {
        closures,
        global_data_blocks: bcf.global_data_blocks.clone(),
        globals,
        primitives: bcf.primitives.clone(),
    })
}

#[derive(Debug)]
struct GlobalCtx {
    instructions: Vec<Instruction<BytecodeRelativeOffset>>,
    referenced_labels: HashSet<usize>,
    label_to_index_map: HashMap<usize, usize>,
    closures_todo: VecDeque<usize>,
    closure_nums: HashMap<usize, usize>,
    parent_closures: HashMap<usize, usize>,
}

impl GlobalCtx {
    fn get_closure(&mut self, label: usize) -> usize {
        match self.closure_nums.get(&label).copied() {
            Some(n) => n,
            None => {
                self.closures_todo.push_back(label);
                let new_closure_no = self.closure_nums.len();
                self.closure_nums.insert(label, new_closure_no);
                new_closure_no
            }
        }
    }

    fn lookup_label(&self, label: usize) -> Result<usize> {
        match self.label_to_index_map.get(&label) {
            Some(index) => Ok(*index),
            None => bail!("Could not find label {} defined", label),
        }
    }

    fn is_block_start(&self, label: usize) -> bool {
        self.referenced_labels.contains(&label)
    }

    fn set_defining_closure(
        &mut self,
        defining_closure: usize,
        defined_closure: usize,
    ) -> Result<()> {
        match self
            .parent_closures
            .insert(defined_closure, defining_closure)
        {
            None => Ok(()),
            Some(other) => Err(anyhow!(
                "Closure {} defined in both {} and {}",
                defined_closure,
                other,
                defining_closure
            )),
        }
    }
}

fn process_closure(global_ctx: &mut GlobalCtx, entrypoint: usize) -> Result<Closure> {
    let mut closure_ctx = {
        let mut todo = VecDeque::new();
        let mut block_nums = HashMap::new();
        todo.push_back(entrypoint);
        block_nums.insert(entrypoint, 0);

        let current_closure_id = global_ctx.get_closure(entrypoint);

        ClosureCtx {
            todo,
            block_nums,
            current_closure_id,
        }
    };

    let mut blocks = Vec::new();
    while let Some(block_entrypoint) = closure_ctx.todo.pop_front() {
        blocks.push(process_block(
            global_ctx,
            &mut closure_ctx,
            block_entrypoint,
        )?);
    }

    Ok(Closure { blocks })
}

#[derive(Debug)]
struct ClosureCtx {
    todo: VecDeque<usize>,
    block_nums: HashMap<usize, usize>,
    current_closure_id: usize,
}

impl ClosureCtx {
    fn get_block(&mut self, label: usize) -> usize {
        match self.block_nums.get(&label).copied() {
            Some(n) => n,
            None => {
                self.todo.push_back(label);
                let new_block_no = self.block_nums.len();
                self.block_nums.insert(label, new_block_no);
                new_block_no
            }
        }
    }
}

fn process_block(
    global_ctx: &mut GlobalCtx,
    closure_ctx: &mut ClosureCtx,
    first_label: usize,
) -> Result<Block> {
    let start_index = global_ctx.lookup_label(first_label)?;
    let mut current_index = start_index;
    let mut block_instructions = Vec::new();

    let mut closures = Vec::new();
    let mut traps = Vec::new();
    let mut end = None;

    while end.is_none() {
        let current_instruction = &global_ctx.instructions[current_index].clone();
        match current_instruction {
            // If this is a block boundary determined earlier, emit as an unconditional jump
            Instruction::LabelDef(l) => {
                let current_label = l.0;
                if current_index != start_index && global_ctx.is_block_start(current_label) {
                    let next_block = closure_ctx.get_block(l.0);
                    end = Some(BlockExit::UnconditionalJump(next_block));
                    break;
                }
            }

            // Otherwise, parse the instruction and map labels to

            // Closures are done with a global (non-block specific counter)
            Instruction::Closure(_, _) | Instruction::ClosureRec(_, _) => {
                let mapped_instruction =
                    current_instruction.map_labels(|l| global_ctx.get_closure(l.0));

                block_instructions.push(mapped_instruction);
            }
            // Otherwise process the instruction mapping labels to internal block ones
            _ => {
                let mapped_instruction =
                    current_instruction.map_labels(|l| closure_ctx.get_block(l.0));

                block_instructions.push(mapped_instruction);
            }
        }

        // Instruction-specific behaviour
        match current_instruction {
            // For closures and traps, update the references
            Instruction::Closure(dest, _) => {
                let dest_id = global_ctx.get_closure(dest.0);
                closures.push(dest_id);
                global_ctx.set_defining_closure(closure_ctx.current_closure_id, dest_id)?;
            }
            Instruction::ClosureRec(dests, _) => {
                for dest in dests {
                    let dest_id = global_ctx.get_closure(dest.0);
                    closures.push(dest_id);
                    global_ctx.set_defining_closure(closure_ctx.current_closure_id, dest_id)?;
                }
            }
            // PushTraps are treated as block boundaries
            Instruction::PushTrap(dest) => {
                traps.push(closure_ctx.get_block(dest.0));

                let next_label = match &global_ctx.instructions[current_index + 1] {
                    Instruction::LabelDef(l) => l.0,
                    _ => panic!("PushTrap should always be followed by label defs"),
                };

                end = Some(BlockExit::UnconditionalJump(
                    closure_ctx.get_block(next_label),
                ));
            }
            // In other cases, emit end cases
            Instruction::ApplyTerm(_, _) => {
                end = Some(BlockExit::TailCall);
            }
            Instruction::Return(_) => {
                end = Some(BlockExit::Return);
            }
            Instruction::Stop => {
                end = Some(BlockExit::Stop);
            }
            Instruction::Raise(_) => {
                end = Some(BlockExit::Raise);
            }
            Instruction::Branch(dest) => {
                end = Some(BlockExit::UnconditionalJump(closure_ctx.get_block(dest.0)));
            }
            Instruction::BranchIf(dest) => {
                let next_label = match &global_ctx.instructions[current_index + 1] {
                    Instruction::LabelDef(l) => l.0,
                    _ => panic!("Conditional branches should always be followed by label defs"),
                };

                end = Some(BlockExit::ConditionalJump(
                    closure_ctx.get_block(dest.0),
                    closure_ctx.get_block(next_label),
                ));
            }
            Instruction::BranchIfNot(dest) => {
                let next_label = match &global_ctx.instructions[current_index + 1] {
                    Instruction::LabelDef(l) => l.0,
                    _ => panic!("Conditional branches should always be followed by label defs"),
                };

                end = Some(BlockExit::ConditionalJump(
                    closure_ctx.get_block(dest.0),
                    closure_ctx.get_block(next_label),
                ));
            }
            Instruction::BranchCmp(_, _, dest) => {
                let next_label = match &global_ctx.instructions[current_index + 1] {
                    Instruction::LabelDef(l) => l.0,
                    _ => panic!("Conditional branches should always be followed by label defs"),
                };

                end = Some(BlockExit::ConditionalJump(
                    closure_ctx.get_block(dest.0),
                    closure_ctx.get_block(next_label),
                ));
            }
            Instruction::Switch(a_instrs, b_instrs) => {
                let mut dest_blocks = Vec::new();

                for dest in a_instrs {
                    dest_blocks.push(closure_ctx.get_block(dest.0));
                }

                for dest in b_instrs {
                    dest_blocks.push(closure_ctx.get_block(dest.0));
                }

                end = Some(BlockExit::Switch(dest_blocks));
            }
            Instruction::Restart => {
                let next_label = match &global_ctx.instructions[current_index + 1] {
                    Instruction::LabelDef(l) => l.0,
                    _ => panic!("Restart should always be followed by label defs"),
                };
                end = Some(BlockExit::UnconditionalJump(
                    closure_ctx.get_block(next_label),
                ));
            }
            Instruction::Break | Instruction::Event => {
                unreachable!();
            }
            _ => (),
        }

        current_index += 1;
    }

    let exit = end.unwrap();

    Ok(Block {
        instructions: block_instructions,
        closures,
        traps,
        exit,
    })
}
