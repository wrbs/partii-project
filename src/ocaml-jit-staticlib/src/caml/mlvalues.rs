// Type aliases for OCaml stuff to allow adding some semblance of memory safety to this stuff
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Value(pub i64);

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct HeaderPointer(pub *mut u64);

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Tag(pub u8);

impl Value {
    pub const fn from_i64(i: i64) -> Value {
        Value(((i as u64) << 1) as i64 + 1)
    }
    pub const UNIT: Value = Value::from_i64(0);
}

impl HeaderPointer {
    pub fn to_value(&self) -> Value {
        Value(self.0 as i64)
    }
}

// Atoms table

extern "C" {
    static mut caml_atom_table: [u64; 255];
}

impl Value {
    pub fn atom(tag: Tag) -> Value {
        // This is safe in the context of the runtime, because this table is statically allocated
        // once
        unsafe { HeaderPointer(&mut caml_atom_table[tag.0 as usize]).to_value() }
    }
}
