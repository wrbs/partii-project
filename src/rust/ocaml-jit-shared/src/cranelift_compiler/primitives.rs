pub struct Primitives<T> {
    pub caml_fatal_error: T,
}

pub const PRIMITIVE_NAMES: Primitives<&'static str> = Primitives {
    caml_fatal_error: "caml_fatal_error",
};
