use crate::caml::mlvalues::Value;
use dynasmrt::ExecutableBuffer;
use ocaml_jit_shared::Instruction;

const CODE_SIZE: usize = 4; // i32

pub type EntryPoint = extern "C" fn() -> Value;

pub struct CompilerData {
    pub sections: Vec<Section>,
}

pub struct Section {
    pub section_number: usize,
    pub base_address: usize,
    pub length: usize,
    pub entrypoint: EntryPoint,
    pub instructions: Vec<Instruction<usize>>,
    pub compiled_code: ExecutableBuffer,
}

impl CompilerData {
    pub fn initialise() -> CompilerData {
        CompilerData {
            sections: Vec::new(),
        }
    }

    pub fn get_section_for_code(&self, code: &[i32]) -> Option<&Section> {
        let base_address = code.as_ptr() as usize;
        let length = code.len();

        self.sections
            .iter()
            .find(|s| s.base_address == base_address && s.length == length)
    }

    pub fn find_section_for_address(&self, address: usize) -> Option<&Section> {
        self.sections.iter().find(|s| {
            s.base_address <= address && address < s.base_address + (s.length * CODE_SIZE)
        })
    }

    pub fn translate_bytecode_address(&self, address: usize) -> Option<(usize, usize)> {
        self.find_section_for_address(address).map(|s| {
            let offset = (address - s.base_address) / CODE_SIZE;
            (s.section_number, offset)
        })
    }
}

impl Section {
    pub fn new(
        section_number: usize,
        bytecode: &[i32],
        compiled_code: ExecutableBuffer,
        entrypoint: EntryPoint,
        instructions: Vec<Instruction<usize>>,
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
