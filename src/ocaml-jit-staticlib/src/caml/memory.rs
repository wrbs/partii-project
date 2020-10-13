use crate::caml::mlvalues::{BlockValue, Tag, Value};

extern "C" {
    fn jit_support_alloc_small(wosize: i64, tag: u8) -> BlockValue;
    fn caml_initialize(fp: *mut Value, value: Value);
    fn caml_modify(fp: *mut Value, value: Value);
    fn caml_alloc_shr(wosize: i64, tag: u8) -> BlockValue;
}

pub fn alloc_small(wosize: usize, tag: Tag) -> BlockValue {
    unsafe { jit_support_alloc_small(wosize as i64, tag.0) }
}

pub fn alloc_shr(wosize: usize, tag: Tag) -> BlockValue {
    unsafe { caml_alloc_shr(wosize as i64, tag.0) }
}

impl BlockValue {
    // This is used when initialising when alloc_small isn't used
    // otherwise set_field is fine
    pub fn initialize_field(&self, index: usize, value: Value) {
        unsafe { caml_initialize(self.field_pointer(index), value) }
    }

    pub fn modify_field(&self, index: usize, value: Value) {
        unsafe { caml_modify(self.field_pointer(index), value) }
    }
}
