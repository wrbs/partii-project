// Type aliases for OCaml stuff to allow adding some semblance of memory safety to this stuff

/*******************
 * Types of values *
 *******************/

// Reminder - in OCaml ints are 63 bits with the lsb always 1, and heap pointers have 0

// A value is just an i64
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Value(pub i64);

// For specific types of values for type safety - they both can be values
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct LongValue(pub i64);

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct BlockValue(pub i64);

// Either specific type of value can take the place of a value
impl From<LongValue> for Value {
    fn from(x: LongValue) -> Self {
        Value(x.0)
    }
}

impl From<BlockValue> for Value {
    fn from(x: BlockValue) -> Self {
        Value(x.0)
    }
}

// But we need to check when casting to long or block
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ValueType {
    Long(LongValue),
    Block(BlockValue),
}

impl Value {
    pub fn is_long(&self) -> bool {
        self.0 & 1 != 0
    }

    pub fn is_block(&self) -> bool {
        !self.is_long()
    }

    pub fn decode_type(&self) -> ValueType {
        if self.is_long() {
            ValueType::Long(LongValue(self.0))
        } else {
            ValueType::Block(BlockValue(self.0))
        }
    }

    pub fn as_long(&self) -> LongValue {
        debug_assert!(self.is_long());

        LongValue(self.0)
    }

    pub fn as_block(&self) -> BlockValue {
        debug_assert!(self.is_block());

        BlockValue(self.0)
    }
}

#[cfg(test)]
mod cast_tests {
    use super::*;

    #[test]
    fn test_casts() {
        let mut v: Value;

        v = LongValue(3).into();
        assert_eq!(v, Value(3));
        assert!(v.is_long());
        assert!(!v.is_block());
        assert_eq!(v.decode_type(), ValueType::Long(LongValue(3)));

        v = BlockValue(0).into();
        assert_eq!(v, Value(0));
        assert!(v.is_block());
        assert!(!v.is_long());
        assert_eq!(v.decode_type(), ValueType::Block(BlockValue(0)));
    }
}

/* Long values */

// Conversion
impl LongValue {
    #[inline]
    const fn from_i64(i: i64) -> LongValue {
        LongValue((((i as u64) << 1) as i64) + 1)
    }

    #[inline]
    fn to_i64(&self) -> i64 {
        self.0 >> 1
    }
}

impl From<i64> for LongValue {
    #[inline]
    fn from(i: i64) -> Self {
        LongValue::from_i64(i)
    }
}

impl From<LongValue> for i64 {
    #[inline]
    fn from(i: LongValue) -> Self {
        i.to_i64()
    }
}

// Constants

impl LongValue {
    // Constants
    pub const UNIT: LongValue = LongValue::from_i64(0);
}

/* Block values */

// Headers

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Tag(pub u8);

impl Tag {
    pub const CLOSURE: Tag = Tag(247);
    pub const INFIX: Tag = Tag(249);
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Header(pub u64);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum Color {
    White = 0,
    Gray = 1,
    Blue = 2,
    Black = 3,
}

impl Color {
    fn mask(&self) -> u64 {
        (*self as u64) << 8
    }
}

impl Header {
    pub fn make(wosize: usize, tag: Tag, color: Color) -> Header {
        Header((wosize as u64) << 10 | color.mask() | tag.0 as u64)
    }

    pub fn tag(&self) -> Tag {
        // This truncates in Rust to just the bottom 8 bits (as we want)
        Tag(self.0 as u8)
    }

    // There's no spacetime profiling so it's easy
    pub fn wosize(&self) -> usize {
        (self.0 >> 10) as usize
    }
}

impl From<Header> for Value {
    fn from(h: Header) -> Self {
        Value(h.0 as i64)
    }
}

// This stuff is highly unsafe but we have to live with it to be pragmatic in ocaml
impl BlockValue {
    pub fn field_pointer(&self, index: usize) -> *mut Value {
        unsafe { (self.0 as *mut Value).add(index) }
    }

    pub fn header_pointer(&self) -> *mut Header {
        unsafe { (self.0 as *mut Header).offset(-1) }
    }

    pub fn header(&self) -> Header {
        unsafe { *self.header_pointer() }
    }

    pub fn set_header(&self, header: Header) {
        unsafe { (*self.header_pointer()).0 = header.0 }
    }

    pub fn tag(&self) -> Tag {
        self.header().tag()
    }

    pub fn wosize(&self) -> Tag {
        self.header().tag()
    }

    pub fn get_field(&self, index: usize) -> Value {
        unsafe { *self.field_pointer(index) }
    }

    // This should only be used when a small block is allocated
    // In other cases use modify and initialize (in memory.rs)
    pub fn set_field_small(&self, index: usize, value: Value) {
        unsafe { *self.field_pointer(index) = value }
    }
}

// Globals
extern "C" {
    static caml_global_data: BlockValue;
}

impl BlockValue {
    pub fn globals() -> BlockValue {
        unsafe { caml_global_data }
    }
}

// Atoms table
extern "C" {
    static mut caml_atom_table: [u64; 255];
}

impl BlockValue {
    pub fn atom(tag: Tag) -> Value {
        // This is safe in the context of the runtime, because this table is statically allocated
        // once
        unsafe { Value(caml_atom_table[tag.0 as usize] as i64) }
    }
}
