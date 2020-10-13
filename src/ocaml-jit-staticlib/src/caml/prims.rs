use crate::caml::misc::ExtTable;
use crate::caml::mlvalues::Value;

extern "C" {
    static caml_prim_table: ExtTable;
}

// Horrifically unsafe but we live with it

pub unsafe fn call_prim_1(n: usize, a: Value) -> Value {
    let f: extern "C" fn(a: Value) -> Value = std::mem::transmute(caml_prim_table.get(n));
    f(a)
}

pub unsafe fn call_prim_2(n: usize, a: Value, b: Value) -> Value {
    let f: extern "C" fn(a: Value, b: Value) -> Value = std::mem::transmute(caml_prim_table.get(n));
    f(a, b)
}

pub unsafe fn call_prim_3(n: usize, a: Value, b: Value, c: Value) -> Value {
    let f: extern "C" fn(a: Value, b: Value, c: Value) -> Value =
        std::mem::transmute(caml_prim_table.get(n));
    f(a, b, c)
}

pub unsafe fn call_prim_4(n: usize, a: Value, b: Value, c: Value, d: Value) -> Value {
    let f: extern "C" fn(a: Value, b: Value, c: Value, d: Value) -> Value =
        std::mem::transmute(caml_prim_table.get(n));
    f(a, b, c, d)
}

pub unsafe fn call_prim_5(n: usize, a: Value, b: Value, c: Value, d: Value, e: Value) -> Value {
    let f: extern "C" fn(a: Value, b: Value, c: Value, d: Value, e: Value) -> Value =
        std::mem::transmute(caml_prim_table.get(n));
    f(a, b, c, d, e)
}

pub unsafe fn call_prim_n(n: usize, ptr: *const Value, nargs: u32) -> Value {
    let f: extern "C" fn(ptr: *const Value, nargs: u32) -> Value =
        std::mem::transmute(caml_prim_table.get(n));
    f(ptr, nargs)
}
