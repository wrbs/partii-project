use crate::caml::mlvalues::Value;
use crate::compiler::emit_code::emit_longjmp_entrypoint;
use dynasmrt::ExecutableBuffer;
use ocaml_jit_shared::{BytecodeLocation, BytecodeRelativeOffset, Instruction};
use std::ffi::c_void;

const CODE_SIZE: usize = 4; // i32

pub type EntryPoint = extern "C" fn(initial_state: *const c_void) -> Value;
pub type LongjmpEntryPoint =
    extern "C" fn(initial_state: *const c_void, initial_pc: Value) -> Value;

pub struct LongjmpHandler {
    pub compiled_code: ExecutableBuffer,
    pub entrypoint: LongjmpEntryPoint,
}

pub struct CompilerData {
    pub sections: Vec<Option<Section>>,
    pub longjmp_handler: Option<LongjmpHandler>,
    pub callback_compiled: bool,
}

pub struct Section {
    pub section_number: usize,
    pub base_address: usize,
    pub length: usize,
    pub instructions: Vec<Instruction<BytecodeRelativeOffset>>,
    pub compiled_code: ExecutableBuffer,
    pub entrypoint: EntryPoint,
    pub first_instruction_location: usize,
}

impl CompilerData {
    pub fn initialise() -> CompilerData {
        CompilerData {
            sections: Vec::new(),
            longjmp_handler: None,
            callback_compiled: false,
        }
    }

    pub fn get_longjmp_handler(&mut self) -> LongjmpEntryPoint {
        self.longjmp_handler
            .get_or_insert_with(emit_longjmp_entrypoint)
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
        instructions: Vec<Instruction<BytecodeRelativeOffset>>,
        first_instruction_location: usize,
    ) -> Section {
        Section {
            section_number,
            base_address: bytecode.as_ptr() as usize,
            length: bytecode.len(),
            entrypoint,
            instructions,
            compiled_code,
            first_instruction_location,
        }
    }
}
