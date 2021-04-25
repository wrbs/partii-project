Instruction::BranchCmp(cmp, i, l) => {
    let label = self.get_label(l);
    oc_dynasm!(self.ops
        ; mov eax, caml_i32_of_int(*i as i64)  // convert integer to ocaml
                                               // representation
        ; movsxd rcx, eax                      // sign extend to 64 bits
        ; cmp rcx, r_accu                      // compare with accumulator
    );
    match cmp {
        Comp::Eq => {
            oc_dynasm!(self.ops
                ; je =>label                   // jump if equal to label
            );
        }
        Comp::Ne => {
            oc_dynasm!(self.ops
                ; jne =>label                  // jump if not equal to label
            );
        }
        // .. other cases similar
    }
}
