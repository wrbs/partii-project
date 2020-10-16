use crate::interp::loaded_code_lookup::LoadedCodeLookup;
use ocaml_jit_shared::Instruction;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::env;
use std::sync::{Mutex, MutexGuard};

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

pub struct GlobalData {
    pub lookup: LoadedCodeLookup,
    pub instructions: Vec<Instruction<usize>>,
    pub use_new_interpreter: bool,
    pub trace: bool,
}

static GLOBAL_DATA: Lazy<Mutex<GlobalData>> = Lazy::new(|| Mutex::new(GlobalData::new()));

impl GlobalData {
    fn new() -> GlobalData {
        GlobalData {
            lookup: LoadedCodeLookup::new(),
            instructions: Vec::new(),
            use_new_interpreter: env::var("OLD_INTERP").is_err(),
            trace: !env::var("TRACE").is_err(),
        }
    }

    pub fn get() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.try_lock().expect("Global data already locked")
    }

    pub fn translate_pc(&self, pointer: *const i32) -> Option<usize> {
        self.lookup.lookup(pointer).map(|(start, _)| start)
    }

    pub fn translate_pc_exn(&self, pointer: *const i32) -> usize {
        match self.translate_pc(pointer) {
            Some(v) => v,
            None => panic!("Could not find loaded instruction at {:?}", pointer),
        }
    }
}
