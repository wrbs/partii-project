use crate::caml::mlvalues::Value;
use dynasmrt::ExecutableBuffer;
use ocaml_jit_shared::Instruction;

pub type EntryPoint = extern "C" fn() -> Value;

pub struct CompilerData {
    pub sections: Vec<Section>,
}

pub struct Section {
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

    pub fn get_section(&self, code: &[i32]) -> Option<&Section> {
        let base_address = code.as_ptr() as usize;
        let length = code.len();

        self.sections
            .iter()
            .find(|s| s.base_address == base_address && s.length == length)
    }
}

impl Section {
    pub fn new(
        bytecode: &[i32],
        compiled_code: ExecutableBuffer,
        entrypoint: EntryPoint,
        instructions: Vec<Instruction<usize>>,
    ) -> Section {
        Section {
            base_address: bytecode.as_ptr() as usize,
            length: bytecode.len(),
            entrypoint,
            instructions,
            compiled_code,
        }
    }
}
