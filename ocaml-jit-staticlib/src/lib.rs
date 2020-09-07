mod caml;

#[no_mangle]
pub extern "C" fn caml_main(args: *mut *mut caml::char_os) {
    unsafe { caml::byt_main(args); }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
