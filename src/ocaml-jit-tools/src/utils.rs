use std::fmt::Display;
use std::process::exit;

pub fn die<D: Display, A>(error: D) -> A {
    eprintln!("Fatal: {}", error);
    exit(1);
}
