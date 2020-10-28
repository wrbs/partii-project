use crate::compiler::CompilerData;
use crate::configuration::Options;
use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};

pub struct GlobalData {
    pub options: Options,
    pub compiler_data: CompilerData,
}

static GLOBAL_DATA: Lazy<Mutex<GlobalData>> = Lazy::new(|| Mutex::new(GlobalData::new()));

impl GlobalData {
    fn new() -> GlobalData {
        GlobalData {
            options: Options::get_from_env(),
            compiler_data: CompilerData::initialise(),
        }
    }

    pub fn get() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.try_lock().expect("Global data already locked")
    }
}
