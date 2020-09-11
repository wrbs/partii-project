use crate::ffi::mlvalues::Value;
use chrono::Utc;
use ocaml_jit_shared::parse_instructions;
use std::env;
use std::fs;
use std::io::{BufWriter, Write};
use std::slice;
use std::time::Instant;

extern "C" {
    fn actual_caml_interprete(prog: *const i32, prog_size: usize) -> Value;
}

#[no_mangle]
pub extern "C" fn caml_interprete(prog: *const i32, prog_size: usize) -> Value {
    unsafe { actual_caml_interprete(prog, prog_size) }
}

#[no_mangle]
pub extern "C" fn caml_prepare_bytecode(prog: *const i32, prog_size: usize) {
    if env::var("PRINT_INSTRS") == Ok("1".to_string()) {
        let now = Instant::now();
        let mut f = BufWriter::new(fs::File::create(format!("/tmp/dis_{}", Utc::now())).unwrap());
        let code_slice = unsafe { slice::from_raw_parts(prog, prog_size / 4) };

        let e1;
        match parse_instructions(code_slice.iter().copied()) {
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

#[no_mangle]
pub extern "C" fn caml_release_bytecode(_prog: *const i32, _prog_size: usize) {}
