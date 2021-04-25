Instruction::BranchCmp(cmp, i, l) => {
    let label = self.get_label(l);
    self.ops.extend(b"\xb8");
    self.ops.push_i32(caml_i32_of_int(*i as i64));
    self.ops.extend(b"Hc\xc8I;\xcd");

    match cmp {
        Comp::Eq => {
            self.ops.extend(b"\x0f\x84\x00\x00\x00\x00");
            // encodes PC-relative 4 byte dynamic relocation to `label`
            self.ops.dynamic_reloc(label, 0isize, 4u8, 0u8, (4u8, 0u8));
        }
        // ...
    }
}
