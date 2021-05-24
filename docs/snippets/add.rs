Instruction::ArithInt(ArithOp::Add) => {
    oc_dynasm!(self.ops
        ; add r_accu, [r_sp]    ; r_sp is alias for r13
        ; dec r_accu            ; r_accu is alias for r15
        ; add r_sp, BYTE 8
    );
}