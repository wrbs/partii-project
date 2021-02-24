use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;

use ocaml_jit_shared::Opcode;

use crate::compiler::CompilerData;
use crate::configuration::Options;
use crate::on_startup;

pub struct GlobalData {
    pub options: Options,
    pub compiler_data: CompilerData,
    pub instruction_counts: Option<HashMap<Opcode, usize>>,
}

static GLOBAL_DATA: Lazy<Mutex<GlobalData>> = Lazy::new(|| Mutex::new(on_startup()));

impl GlobalData {
    pub fn new() -> GlobalData {
        let options = Options::get_from_env();

        let instruction_counts = if options.save_instruction_counts {
            Some(HashMap::new())
        } else {
            None
        };

        GlobalData {
            options,
            compiler_data: CompilerData::initialise(),
            instruction_counts,
        }
    }

    pub fn get() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.try_lock().expect("Global data already locked")
    }
}
