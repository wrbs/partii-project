use crate::caml::mlvalues::Value;
use chrono::Utc;
use ocaml_jit_shared::parse_instructions;
use std::env;
use std::fs;
use std::io::{BufWriter, Write};
use std::time::Instant;

extern "C" {
    fn actual_caml_interprete(prog: *const i32, prog_size: usize) -> Value;
}

pub fn interpret_bytecode(code: &[i32]) -> Value {
    unsafe { actual_caml_interprete(code.as_ptr(), code.len()) }
}

pub fn on_bytecode_loaded(code: &[i32]) {
    if env::var("PRINT_INSTRS") == Ok("1".to_string()) {
        let now = Instant::now();
        let mut f = BufWriter::new(fs::File::create(format!("/tmp/dis_{}", Utc::now())).unwrap());

        let e1;
        match parse_instructions(code.iter().copied()) {
            Some(instructions) => {
                e1 = now.elapsed();
                for (index, instruction) in instructions {
                    writeln!(f, "{:10}: {:?}", index, instruction).unwrap();
                }
            }
            None => {
                e1 = now.elapsed();
                writeln!(f, "Error when decoding").unwrap();
            }
        }

        writeln!(
            f,
            "Time taken: {} {}",
            e1.as_secs_f32(),
            now.elapsed().as_secs_f32()
        )
        .unwrap();
    }
}

pub fn on_bytecode_released(_code: &[i32]) {}
