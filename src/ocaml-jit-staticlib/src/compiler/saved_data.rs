use crate::caml::mlvalues::Value;
use dynasmrt::ExecutableBuffer;
use ocaml_jit_shared::{BytecodeLocation, BytecodeRelativeOffset, Instruction};

const CODE_SIZE: usize = 4; // i32

pub type EntryPoint = extern "C" fn() -> Value;

pub struct CompilerData {
    pub sections: Vec<Option<Section>>,
}

pub struct Section {
    pub section_number: usize,
    pub base_address: usize,
    pub length: usize,
    pub entrypoint: EntryPoint,
    pub instructions: Vec<Instruction<BytecodeRelativeOffset>>,
    pub compiled_code: ExecutableBuffer,
}

impl CompilerData {
    pub fn initialise() -> CompilerData {
        CompilerData {
            sections: Vec::new(),
        }
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
        self.sections[section_number] = None;
    }
}

impl Section {
    pub fn new(
        section_number: usize,
        bytecode: &[i32],
        compiled_code: ExecutableBuffer,
        entrypoint: EntryPoint,
        instructions: Vec<Instruction<BytecodeRelativeOffset>>,
    ) -> Section {
        Section {
            section_number,
            base_address: bytecode.as_ptr() as usize,
            length: bytecode.len(),
            entrypoint,
            instructions,
            compiled_code,
        }
    }
}
