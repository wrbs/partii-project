Instruction::ArithInt(ArithOp::Add) => {
    oc_dynasm!(self.ops
        ; add r_accu, [r_sp]
        ; dec r_accu
        ; add r_sp, BYTE 8
    );
}