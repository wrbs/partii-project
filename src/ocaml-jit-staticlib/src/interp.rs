use crate::caml::mlvalues::Value;

extern "C" {
    fn actual_caml_interprete(prog: *const i32, prog_size: usize) -> Value;
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    unsafe { actual_caml_interprete(code.as_ptr(), code.len()) }
}

pub fn on_bytecode_loaded(_code: &[i32]) {}

pub fn on_bytecode_released(_code: &[i32]) {}
