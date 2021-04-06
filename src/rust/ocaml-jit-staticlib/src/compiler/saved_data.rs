use std::collections::HashMap;

use dynasmrt::ExecutableBuffer;

use ocaml_jit_shared::{BytecodeLocation, BytecodeRelativeOffset, Instruction};

use crate::caml::mlvalues::Value;

use super::{emit_code::emit_cranelift_callback_entrypoint, CompilerOptions};

const CODE_SIZE: usize = 4; // i32

pub type EntryPoint = unsafe extern "C" fn() -> Value;

pub struct AsmCompiledPrimitive<T> {
    pub compiled_code: ExecutableBuffer,
    pub entrypoint: T,
}

pub struct CraneliftApplyAddresses {
    pub apply_1: usize,
    pub apply_2: usize,
    pub apply_3: usize,
    pub apply_4: usize,
    pub apply_5: usize,
    pub apply_n: usize,
    pub return_addr: usize,
}

pub struct CompilerData {
    pub sections: Vec<Option<Section>>,
    pub cranelift_apply: Option<AsmCompiledPrimitive<CraneliftApplyAddresses>>,
    pub callback_compiled: bool,
    pub compiler_options: CompilerOptions,
}

pub struct Section {
    pub section_number: usize,
    pub base_address: usize,
    pub length: usize,
    // This is only filled when it is needed for traces to avoid wasting space
    pub instructions: Option<Vec<Instruction<BytecodeRelativeOffset>>>,
    pub compiled_code: ExecutableBuffer,
    pub entrypoint: EntryPoint,
    pub first_instruction_location: usize,
    pub closure_addresses: HashMap<usize, usize>,
}

impl CompilerData {
    pub fn new(compiler_options: CompilerOptions) -> Self {
        Self {
            sections: Vec::new(),
            cranelift_apply: None,
            callback_compiled: false,
            compiler_options,
        }
    }
    pub fn get_cranelift_apply_addresses(&mut self) -> &CraneliftApplyAddresses {
        let options = self.compiler_options;
        &self
            .cranelift_apply
            .get_or_insert_with(|| emit_cranelift_callback_entrypoint(options))
            .entrypoint
    }

    fn actual_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter_map(|x| match x {
            Some(s) => Some(s),
            None => None,
        })
    }

    pub fn get_section_for_code(&self, code: &[i32]) -> Option<&Section> {
        let base_address = code.as_ptr() as usize;
        let length = code.len();

        self.actual_sections()
            .find(|s| s.base_address == base_address && s.length == length)
    }

    pub fn find_section_for_address(&self, address: usize) -> Option<&Section> {
        self.actual_sections().find(|s| {
            s.base_address <= address && address < s.base_address + (s.length * CODE_SIZE)
        })
    }

    pub fn translate_bytecode_address(&self, address: usize) -> Option<BytecodeLocation> {
        self.find_section_for_address(address).map(|s| {
            let offset = (address - s.base_address) / CODE_SIZE;
            BytecodeLocation {
                section_number: s.section_number,
                offset: BytecodeRelativeOffset(offset),
            }
        })
    }

    pub fn release_section(&mut self, code: &[i32]) {
        let section_number = self.get_section_for_code(code).unwrap().section_number;
        if section_number == self.sections.len() - 1 {
            let _ = self.sections.pop().unwrap();
        } else {
            self.sections[section_number] = None;
        }
    }
}

impl Section {
    pub fn new(
        section_number: usize,
        bytecode: &[i32],
        compiled_code: ExecutableBuffer,
        entrypoint: EntryPoint,
        instructions: Option<Vec<Instruction<BytecodeRelativeOffset>>>,
        first_instruction_location: usize,
        closure_addresses: HashMap<usize, usize>,
    ) -> Section {
        Section {
            section_number,
            base_address: bytecode.as_ptr() as usize,
            length: bytecode.len(),
            entrypoint,
            instructions,
            compiled_code,
            first_instruction_location,
            closure_addresses,
        }
    }

    pub unsafe fn get_code<'a>(&self) -> &'a [i32] {
        std::slice::from_raw_parts(self.base_address as *const i32, self.length)
    }
}
