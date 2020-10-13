extern "C" {
    fn caml_raise_zero_divide();
}

pub fn raise_zero_divide() {
    unsafe { caml_raise_zero_divide() }
}
