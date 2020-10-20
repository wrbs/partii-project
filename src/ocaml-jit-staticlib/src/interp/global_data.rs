use ocaml_jit_shared::Instruction;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, MutexGuard};
use crate::caml::mlvalues::Value;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct CodePtr(pub usize);

impl CodePtr {
    pub fn from(ptr: *const i32) -> CodePtr {
        CodePtr(ptr as usize)
    }

    pub fn as_ptr(&self) -> *const i32 {
        self.0 as *const i32
    }
}

pub struct Section {
    pub base_address: usize,
    pub length: usize,
    pub entrypoint: extern fn () -> Value,
    pub instructions: Vec<Instruction<usize>>,
}

pub struct GlobalData {
    pub use_new_interpreter: bool,
    pub trace: bool,
    pub sections: Vec<Section>
}

static GLOBAL_DATA: Lazy<Mutex<GlobalData>> = Lazy::new(|| Mutex::new(GlobalData::new()));

impl GlobalData {
    fn new() -> GlobalData {
        GlobalData {
            use_new_interpreter: env::var("OLD_INTERP").is_err(),
            trace: !env::var("TRACE").is_err(),
            sections: Vec::new()
        }
    }

    pub fn get() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.try_lock().expect("Global data already locked")
    }

    pub fn find_section(&self, code: &[i32]) -> Option<&Section> {
        let base_address = code.as_ptr() as usize;
        let length = code.len();

        self.sections.iter().find(|s| s.base_address == base_address && s.length == length)
    }
}

impl Section {
    pub fn execute(&self) -> Value {
        unsafe { self.entrypoint() }
    }
}