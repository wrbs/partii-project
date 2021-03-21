use crate::Opcode;

pub struct Closure {
    pub code_offset: usize,
    pub arity: usize,
}

pub enum FoundClosure {
    Normal { func: Closure, nvars: usize },
    Rec { funcs: Vec<Closure>, nvars: usize },
}

// default result type in this file
pub struct ClosureIterator<'code> {
    code: &'code [i32],
    offset: usize,
    position: usize,
    done: bool,
}

impl<'code> ClosureIterator<'code> {
    pub fn new(code: &'code [i32]) -> Self {
        Self::new_from_offset(code, 0)
    }

    pub fn new_from_offset(code: &'code [i32], offset: usize) -> Self {
        Self {
            code,
            offset,
            position: 0,
            done: false,
        }
    }

    fn current_position(&self) -> usize {
        self.position + self.offset
    }

    fn i32(&mut self) -> Option<i32> {
        if self.position == self.code.len() {
            None
        } else {
            let v = self.code[self.position];
            self.position += 1;
            Some(v)
        }
    }

    fn u32(&mut self) -> Option<u32> {
        Some(self.i32()? as u32)
    }

    fn opcode(&mut self) -> Option<Opcode> {
        let v = self.i32()?;
        match Opcode::from_i32(v) {
            Some(x) => Some(x),
            None => None,
        }
    }

    fn label_at(&mut self, position: usize) -> Option<usize> {
        let rel = self.i32()?;
        let location = position as i32 + rel;
        if location < 0 {
            return None;
        }
        Some(location as usize)
    }

    fn label(&mut self) -> Option<usize> {
        self.label_at(self.current_position())
    }

    fn get_closure_at(&self, code_offset: usize) -> Option<Closure> {
        // Every function which takes >= 2 arguments, starts with a grab
        // If it doesn't, it's a 1-arg function
        let arity = {
            let first_word = *self.code.get(code_offset - self.offset)?;
            if let Some(Opcode::Grab) = Opcode::from_i32(first_word) {
                let num_extra = *self.code.get(code_offset - self.offset + 1)?;
                num_extra as usize + 1
            } else {
                1
            }
        };

        Some(Closure { code_offset, arity })
    }

    fn next_closure_impl(&mut self) -> Option<FoundClosure> {
        loop {
            match self.opcode()? {
                // The cases we care about
                Opcode::Closure => {
                    let nvars = self.u32()? as usize;
                    let code_offset = self.label()?;

                    return Some(FoundClosure::Normal {
                        func: self.get_closure_at(code_offset)?,
                        nvars,
                    });
                }
                Opcode::ClosureRec => {
                    let nfuncs = self.u32()? as usize;
                    let nvars = self.u32()? as usize;
                    let mut funcs = vec![];

                    let position = self.current_position();

                    for _ in 0..nfuncs {
                        let code_offset = self.label_at(position)?;
                        funcs.push(self.get_closure_at(code_offset)?)
                    }

                    return Some(FoundClosure::Rec { funcs, nvars });
                }

                // Rest of the cases we don't care - we just need to advance the right amount
                // Nullary
                Opcode::Acc0
                | Opcode::Acc1
                | Opcode::Acc2
                | Opcode::Acc3
                | Opcode::Acc4
                | Opcode::Acc5
                | Opcode::Acc6
                | Opcode::Acc7
                | Opcode::Push
                | Opcode::PushAcc0
                | Opcode::PushAcc1
                | Opcode::PushAcc2
                | Opcode::PushAcc3
                | Opcode::PushAcc4
                | Opcode::PushAcc5
                | Opcode::PushAcc6
                | Opcode::PushAcc7
                | Opcode::EnvAcc1
                | Opcode::EnvAcc2
                | Opcode::EnvAcc3
                | Opcode::EnvAcc4
                | Opcode::PushEnvAcc1
                | Opcode::PushEnvAcc2
                | Opcode::PushEnvAcc3
                | Opcode::PushEnvAcc4
                | Opcode::Apply1
                | Opcode::Apply2
                | Opcode::Apply3
                | Opcode::Restart
                | Opcode::OffsetClosureM2
                | Opcode::OffsetClosure0
                | Opcode::OffsetClosure2
                | Opcode::PushOffsetClosureM2
                | Opcode::PushOffsetClosure0
                | Opcode::PushOffsetClosure2
                | Opcode::Atom0
                | Opcode::PushAtom0
                | Opcode::GetField0
                | Opcode::GetField1
                | Opcode::GetField2
                | Opcode::GetField3
                | Opcode::SetField0
                | Opcode::SetField1
                | Opcode::SetField2
                | Opcode::SetField3
                | Opcode::VecTLength
                | Opcode::GetVecTItem
                | Opcode::SetVecTItem
                | Opcode::GetBytesChar
                | Opcode::SetBytesChar
                | Opcode::BoolNot
                | Opcode::PopTrap
                | Opcode::Raise
                | Opcode::CheckSignals
                | Opcode::Const0
                | Opcode::Const1
                | Opcode::Const2
                | Opcode::Const3
                | Opcode::PushConst0
                | Opcode::PushConst1
                | Opcode::PushConst2
                | Opcode::PushConst3
                | Opcode::NegInt
                | Opcode::AddInt
                | Opcode::SubInt
                | Opcode::MulInt
                | Opcode::DivInt
                | Opcode::ModInt
                | Opcode::AndInt
                | Opcode::OrInt
                | Opcode::XorInt
                | Opcode::LslInt
                | Opcode::LsrInt
                | Opcode::AsrInt
                | Opcode::Eq
                | Opcode::Neq
                | Opcode::LtInt
                | Opcode::LeInt
                | Opcode::GtInt
                | Opcode::GeInt
                | Opcode::IsInt
                | Opcode::GetMethod
                | Opcode::GetDynMet
                | Opcode::Stop
                | Opcode::Event
                | Opcode::Break
                | Opcode::ReRaise
                | Opcode::RaiseNoTrace
                | Opcode::GetStringChar => {
                    // Do nothing
                }

                // Unary
                Opcode::PushAcc
                | Opcode::Acc
                | Opcode::Pop
                | Opcode::Assign
                | Opcode::EnvAcc
                | Opcode::PushEnvAcc
                | Opcode::PushRetAddr
                | Opcode::Apply
                | Opcode::AppTerm1
                | Opcode::AppTerm2
                | Opcode::AppTerm3
                | Opcode::Return
                | Opcode::Grab
                | Opcode::OffsetClosure
                | Opcode::PushOffsetClosure
                | Opcode::GetGlobal
                | Opcode::PushGetGlobal
                | Opcode::SetGlobal
                | Opcode::Atom
                | Opcode::PushAtom
                | Opcode::MakeBlock1
                | Opcode::MakeBlock2
                | Opcode::MakeBlock3
                | Opcode::MakeFloatBlock
                | Opcode::GetField
                | Opcode::GetFloatField
                | Opcode::SetField
                | Opcode::SetFloatField
                | Opcode::Branch
                | Opcode::BranchIf
                | Opcode::BranchIfNot
                | Opcode::PushTrap
                | Opcode::CCall1
                | Opcode::CCall2
                | Opcode::CCall3
                | Opcode::CCall4
                | Opcode::CCall5
                | Opcode::ConstInt
                | Opcode::PushConstInt
                | Opcode::OffsetInt
                | Opcode::OffsetRef => {
                    // Read argument, discard
                    let _ = self.i32()?;
                }

                // Binary
                Opcode::AppTerm
                | Opcode::GetGlobalField
                | Opcode::PushGetGlobalField
                | Opcode::MakeBlock
                | Opcode::CCallN
                | Opcode::BEq
                | Opcode::BNeq
                | Opcode::BLtInt
                | Opcode::BLeInt
                | Opcode::BGtInt
                | Opcode::BGeInt
                | Opcode::ULtInt
                | Opcode::UGeInt
                | Opcode::BULtInt
                | Opcode::BUGeInt
                | Opcode::GetPubMet => {
                    // Read two, discard
                    let _ = self.i32()?;
                    let _ = self.i32()?;
                }

                // Special - switch
                Opcode::Switch => {
                    let n = self.u32()?;
                    let n_ints = n & 0xFFFF;
                    let n_tags = n >> 16;

                    for _ in 0..(n_ints + n_tags) {
                        let _ = self.i32()?;
                    }
                }
            }
        }
    }
}

impl<'code> Iterator for ClosureIterator<'code> {
    type Item = FoundClosure;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            match self.next_closure_impl() {
                Some(res) => Some(res),
                None => {
                    self.done = true;
                    None
                }
            }
        }
    }
}
