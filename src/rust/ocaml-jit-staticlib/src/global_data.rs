use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use once_cell::sync::Lazy;

use ocaml_jit_shared::Opcode;

use crate::{
    compiler::{CompilerData, CompilerOptions, DEFAULT_HOT_CLOSURE_THRESHOLD},
    configuration::Options,
    on_startup,
};

pub struct GlobalData {
    pub options: Options,
    pub compiler_options: CompilerOptions,
    pub compiler_data: CompilerData,
    pub instruction_counts: Option<HashMap<Opcode, usize>>,
}

static GLOBAL_DATA: Lazy<Mutex<GlobalData>> = Lazy::new(|| Mutex::new(on_startup()));

impl GlobalData {
    #[allow(clippy::clippy::new_without_default)]
    pub fn new() -> GlobalData {
        let options = Options::get_from_env();

        let instruction_counts = if options.save_instruction_counts {
            Some(HashMap::new())
        } else {
            None
        };

        let hot_closure_threshold = if options.no_hot_threshold {
            None
        } else {
            options.hot_threshold.or(DEFAULT_HOT_CLOSURE_THRESHOLD)
        };

        let compiler_options = CompilerOptions {
            print_traces: options.trace,
            hot_closure_threshold,
        };

        GlobalData {
            options,
            compiler_options,
            compiler_data: CompilerData::initialise(),
            instruction_counts,
        }
    }

    pub fn get() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.try_lock().expect("Global data already locked")
    }
}
