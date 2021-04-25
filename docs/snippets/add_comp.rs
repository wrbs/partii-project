Instruction::ArithInt(ArithOp::Add) => {
    self.ops.extend(b"M\x03/I\xff\xcdI\x83\xc7");
    self.ops.push_i8(8);
}
