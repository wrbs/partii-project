use std::error::Error;
use std::process::exit;

pub fn die<E: Error, A>(error: E) -> A {
    eprintln!("Fatal: {}", error);
    exit(1);
}

pub fn die_dyn<A>(error: Box<dyn Error>) -> A {
    eprintln!("Fatal: {}", error);
    exit(1);
}
