use super::types::*;
use crate::Opcode;
use std::collections::VecDeque;
use std::iter::Peekable;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstructionParseError {
    #[error("unexpected end of stream")]
    UnexpectedEnd,

    #[error("negative label of {0}")]
    NegativeLabel(i32),

    #[error("unknown opcode: {0}")]
    BadOpcode(i32),
}

// default result type in this file
type Result<T, E = InstructionParseError> = std::result::Result<T, E>;

pub struct InstructionIterator<I: Iterator<Item = i32>> {
    iter: Peekable<I>,
    position: usize,
    next_queued: VecDeque<Instruction<BytecodeRelativeOffset>>,
    error: bool,
}

impl<I: Iterator<Item = i32>> InstructionIterator<I> {
    pub fn new(iterator: I) -> InstructionIterator<I> {
        InstructionIterator {
            iter: iterator.peekable(),
            position: 0,
            next_queued: VecDeque::new(),
            error: false,
        }
    }

    pub fn current_position(&self) -> usize {
        self.position
    }

    fn at_end(&mut self) -> bool {
        self.iter.peek().is_none()
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.i32()? as u8)
    }

    fn i32(&mut self) -> Result<i32> {
        match self.iter.next() {
            Some(v) => {
                self.position += 1;
                Ok(v)
            }
            None => Err(InstructionParseError::UnexpectedEnd),
        }
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(self.i32()? as u32)
    }

    fn opcode(&mut self) -> Result<Opcode> {
        let v = self.i32()?;
        match Opcode::from_i32(v) {
            Some(x) => Ok(x),
            None => Err(InstructionParseError::BadOpcode(v)),
        }
    }

    fn label_at(&mut self, position: usize) -> Result<BytecodeRelativeOffset> {
        let rel = self.i32()?;
        let location = position as i32 + rel;
        if location < 0 {
            return Err(InstructionParseError::NegativeLabel(location));
        }
        Ok(BytecodeRelativeOffset(location as usize))
    }

    fn label(&mut self) -> Result<BytecodeRelativeOffset> {
        self.label_at(self.current_position())
    }

    fn get_label_list(
        &mut self,
        count: usize,
        position: usize,
    ) -> Result<Vec<BytecodeRelativeOffset>> {
        let mut result = Vec::new();
        for _ in 0..count {
            result.push(self.label_at(position)?);
        }

        Ok(result)
    }

    fn get_next_instr(&mut self) -> Result<Option<Instruction<BytecodeRelativeOffset>>> {
        /*
         * The thing that makes this complicated is that we simplify the bytecode format as we load
         * things. Specifically one original bytecode instruction can correspond to multiple
         * simplified instructions:
         *
         * eg.
         * PushAcc => Push, Acc
         * PushGetGlobalField => Push, GetGlobal, GetField
         *
         * for debugging/tracing I want to be able to go from original pointers in the bytecode
         * to the slice of instructions.
         *
         * We store the start here and the end after we've worked out the instruction
         */

        if let Some(i) = self.next_queued.pop_front() {
            return Ok(Some(i));
        }

        if self.at_end() {
            return Ok(None);
        }

        let label_def = Instruction::LabelDef(BytecodeRelativeOffset(self.current_position()));

        // Every bytecode instruction has at least one simplified instruction pushed so we simplify
        // things in most cases by pushing this at the end
        let to_push = match self.opcode()? {
            Opcode::Acc0 => Instruction::Acc(0),
            Opcode::Acc1 => Instruction::Acc(1),
            Opcode::Acc2 => Instruction::Acc(2),
            Opcode::Acc3 => Instruction::Acc(3),
            Opcode::Acc4 => Instruction::Acc(4),
            Opcode::Acc5 => Instruction::Acc(5),
            Opcode::Acc6 => Instruction::Acc(6),
            Opcode::Acc7 => Instruction::Acc(7),
            Opcode::Acc => Instruction::Acc(self.u32()?),

            Opcode::PushAcc0 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(0)
            }
            Opcode::PushAcc1 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(1)
            }
            Opcode::PushAcc2 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(2)
            }
            Opcode::PushAcc3 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(3)
            }
            Opcode::PushAcc4 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(4)
            }
            Opcode::PushAcc5 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(5)
            }
            Opcode::PushAcc6 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(6)
            }
            Opcode::PushAcc7 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(7)
            }
            Opcode::PushAcc => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Acc(self.u32()?)
            }

            Opcode::EnvAcc1 => Instruction::EnvAcc(1),
            Opcode::EnvAcc2 => Instruction::EnvAcc(2),
            Opcode::EnvAcc3 => Instruction::EnvAcc(3),
            Opcode::EnvAcc4 => Instruction::EnvAcc(4),
            Opcode::EnvAcc => Instruction::EnvAcc(self.u32()?),

            Opcode::PushEnvAcc1 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::EnvAcc(1)
            }
            Opcode::PushEnvAcc2 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::EnvAcc(2)
            }
            Opcode::PushEnvAcc3 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::EnvAcc(3)
            }
            Opcode::PushEnvAcc4 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::EnvAcc(4)
            }
            Opcode::PushEnvAcc => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::EnvAcc(self.u32()?)
            }

            Opcode::Push => Instruction::Push,
            Opcode::Pop => Instruction::Pop(self.u32()?),
            Opcode::Assign => Instruction::Assign(self.u32()?),
            Opcode::PushRetAddr => Instruction::PushRetAddr(self.label()?),

            Opcode::Apply1 => Instruction::Apply1,
            Opcode::Apply2 => Instruction::Apply2,
            Opcode::Apply3 => Instruction::Apply3,
            Opcode::Apply => Instruction::Apply(self.u32()?),

            Opcode::AppTerm1 => Instruction::ApplyTerm(1, self.u32()?),
            Opcode::AppTerm2 => Instruction::ApplyTerm(2, self.u32()?),
            Opcode::AppTerm3 => Instruction::ApplyTerm(3, self.u32()?),
            Opcode::AppTerm => Instruction::ApplyTerm(self.u32()?, self.u32()?),

            Opcode::Return => Instruction::Return(self.u32()?),
            Opcode::Restart => Instruction::Restart,
            Opcode::Grab => {
                let pos = self.current_position();
                let prev_restart = pos - 2;
                let n = self.u32()?;
                Instruction::Grab(BytecodeRelativeOffset(prev_restart), n)
            }

            Opcode::Closure => {
                let n = self.u32()?;
                let label = self.label()?;
                Instruction::Closure(label, n)
            }
            Opcode::ClosureRec => {
                let length = self.u32()?;
                let n = self.u32()?;
                let pos = self.current_position();
                let data = self.get_label_list(length as usize, pos)?;
                Instruction::ClosureRec(data, n)
            }

            Opcode::OffsetClosure0 => Instruction::OffsetClosure(0),
            Opcode::OffsetClosure2 => Instruction::OffsetClosure(2),
            Opcode::OffsetClosureM2 => Instruction::OffsetClosure(-2),
            Opcode::OffsetClosure => Instruction::OffsetClosure(self.i32()?),

            Opcode::PushOffsetClosure0 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::OffsetClosure(0)
            }
            Opcode::PushOffsetClosure2 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::OffsetClosure(2)
            }
            Opcode::PushOffsetClosureM2 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::OffsetClosure(-2)
            }
            Opcode::PushOffsetClosure => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::OffsetClosure(self.i32()?)
            }

            Opcode::GetGlobal => Instruction::GetGlobal(self.u32()?),
            Opcode::PushGetGlobal => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::GetGlobal(self.u32()?)
            }
            Opcode::GetGlobalField => {
                let gg = Instruction::GetGlobal(self.u32()?);
                self.next_queued.push_back(gg);
                Instruction::GetField(self.u32()?)
            }
            Opcode::PushGetGlobalField => {
                self.next_queued.push_back(Instruction::Push);
                let gg = Instruction::GetGlobal(self.u32()?);
                self.next_queued.push_back(gg);
                Instruction::GetField(self.u32()?)
            }

            Opcode::SetGlobal => Instruction::SetGlobal(self.u32()?),

            Opcode::GetField0 => Instruction::GetField(0),
            Opcode::GetField1 => Instruction::GetField(1),
            Opcode::GetField2 => Instruction::GetField(2),
            Opcode::GetField3 => Instruction::GetField(3),
            Opcode::GetField => Instruction::GetField(self.u32()?),

            Opcode::SetField0 => Instruction::SetField(0),
            Opcode::SetField1 => Instruction::SetField(1),
            Opcode::SetField2 => Instruction::SetField(2),
            Opcode::SetField3 => Instruction::SetField(3),
            Opcode::SetField => Instruction::SetField(self.u32()?),

            Opcode::GetFloatField => Instruction::GetFloatField(self.u32()?),
            Opcode::SetFloatField => Instruction::SetFloatField(self.u32()?),

            Opcode::VecTLength => Instruction::VecTLength,
            Opcode::GetVecTItem => Instruction::GetVecTItem,
            Opcode::SetVecTItem => Instruction::SetVecTItem,

            Opcode::GetStringChar => Instruction::GetBytesChar,
            Opcode::GetBytesChar => Instruction::GetBytesChar,
            Opcode::SetBytesChar => Instruction::SetBytesChar,

            Opcode::Branch => Instruction::Branch(self.label()?),
            Opcode::BranchIf => Instruction::BranchIf(self.label()?),
            Opcode::BranchIfNot => Instruction::BranchIfNot(self.label()?),

            Opcode::Switch => {
                let n = self.u32()?;

                let pos = self.current_position();
                let ints = self.get_label_list((n & 0xFFFF) as usize, pos)?;
                let tags = self.get_label_list((n >> 16) as usize, pos)?;

                Instruction::Switch(ints, tags)
            }

            Opcode::NegInt => Instruction::NegInt,
            Opcode::AddInt => Instruction::ArithInt(ArithOp::Add),
            Opcode::SubInt => Instruction::ArithInt(ArithOp::Sub),
            Opcode::MulInt => Instruction::ArithInt(ArithOp::Mul),
            Opcode::DivInt => Instruction::ArithInt(ArithOp::Div),
            Opcode::ModInt => Instruction::ArithInt(ArithOp::Mod),
            Opcode::AndInt => Instruction::ArithInt(ArithOp::And),
            Opcode::OrInt => Instruction::ArithInt(ArithOp::Or),
            Opcode::XorInt => Instruction::ArithInt(ArithOp::Xor),
            Opcode::LslInt => Instruction::ArithInt(ArithOp::Lsl),
            Opcode::LsrInt => Instruction::ArithInt(ArithOp::Lsr),
            Opcode::AsrInt => Instruction::ArithInt(ArithOp::Asr),

            Opcode::BoolNot => Instruction::BoolNot,

            Opcode::Eq => Instruction::IntCmp(Comp::Eq),
            Opcode::Neq => Instruction::IntCmp(Comp::Ne),
            Opcode::LtInt => Instruction::IntCmp(Comp::Lt),
            Opcode::LeInt => Instruction::IntCmp(Comp::Le),
            Opcode::GtInt => Instruction::IntCmp(Comp::Gt),
            Opcode::GeInt => Instruction::IntCmp(Comp::Ge),
            Opcode::ULtInt => Instruction::IntCmp(Comp::ULt),
            Opcode::UGeInt => Instruction::IntCmp(Comp::UGe),

            Opcode::BEq => Instruction::BranchCmp(Comp::Eq, self.i32()?, self.label()?),
            Opcode::BNeq => Instruction::BranchCmp(Comp::Ne, self.i32()?, self.label()?),
            Opcode::BLtInt => Instruction::BranchCmp(Comp::Lt, self.i32()?, self.label()?),
            Opcode::BLeInt => Instruction::BranchCmp(Comp::Le, self.i32()?, self.label()?),
            Opcode::BGtInt => Instruction::BranchCmp(Comp::Gt, self.i32()?, self.label()?),
            Opcode::BGeInt => Instruction::BranchCmp(Comp::Ge, self.i32()?, self.label()?),
            Opcode::BULtInt => Instruction::BranchCmp(Comp::ULt, self.i32()?, self.label()?),
            Opcode::BUGeInt => Instruction::BranchCmp(Comp::UGe, self.i32()?, self.label()?),

            Opcode::IsInt => Instruction::IsInt,

            Opcode::OffsetInt => Instruction::OffsetInt(self.i32()?),
            Opcode::OffsetRef => Instruction::OffsetRef(self.i32()?),

            Opcode::GetMethod => Instruction::GetMethod,
            Opcode::GetPubMet => {
                let tag = self.i32()?;
                let _cache = self.u32()?;
                self.next_queued.push_back(Instruction::SetupForPubMet(tag));
                Instruction::GetDynMet
            }

            Opcode::GetDynMet => Instruction::GetDynMet,

            Opcode::CCall1 => Instruction::CCall1(self.u32()?),
            Opcode::CCall2 => Instruction::CCall2(self.u32()?),
            Opcode::CCall3 => Instruction::CCall3(self.u32()?),
            Opcode::CCall4 => Instruction::CCall4(self.u32()?),
            Opcode::CCall5 => Instruction::CCall5(self.u32()?),
            Opcode::CCallN => Instruction::CCallN(self.u32()?, self.u32()?),

            Opcode::Raise => Instruction::Raise(RaiseKind::Regular),
            Opcode::ReRaise => Instruction::Raise(RaiseKind::ReRaise),
            Opcode::RaiseNoTrace => Instruction::Raise(RaiseKind::NoTrace),

            Opcode::PopTrap => Instruction::PopTrap,
            Opcode::PushTrap => Instruction::PushTrap(self.label()?),

            Opcode::CheckSignals => Instruction::CheckSignals,

            Opcode::Atom => Instruction::MakeBlock(0, self.u8()?),
            Opcode::Atom0 => Instruction::MakeBlock(0, 0),

            Opcode::PushAtom0 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::MakeBlock(0, 0)
            }
            Opcode::PushAtom => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::MakeBlock(0, self.u8()?)
            }

            Opcode::MakeBlock1 => Instruction::MakeBlock(1, self.u8()?),
            Opcode::MakeBlock2 => Instruction::MakeBlock(2, self.u8()?),
            Opcode::MakeBlock3 => Instruction::MakeBlock(3, self.u8()?),
            Opcode::MakeBlock => Instruction::MakeBlock(self.u32()?, self.u8()?),
            Opcode::MakeFloatBlock => Instruction::MakeFloatBlock(self.u32()?),

            Opcode::Const0 => Instruction::Const(0),
            Opcode::Const1 => Instruction::Const(1),
            Opcode::Const2 => Instruction::Const(2),
            Opcode::Const3 => Instruction::Const(3),
            Opcode::ConstInt => Instruction::Const(self.i32()?),

            Opcode::PushConst0 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Const(0)
            }
            Opcode::PushConst1 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Const(1)
            }
            Opcode::PushConst2 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Const(2)
            }
            Opcode::PushConst3 => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Const(3)
            }
            Opcode::PushConstInt => {
                self.next_queued.push_back(Instruction::Push);
                Instruction::Const(self.i32()?)
            }

            Opcode::Stop => Instruction::Stop,
            Opcode::Break => Instruction::Break,
            Opcode::Event => Instruction::Event,
        };

        self.next_queued.push_back(to_push);

        Ok(Some(label_def))
    }
}

impl<I: Iterator<Item = i32>> Iterator for InstructionIterator<I> {
    type Item = Result<Instruction<BytecodeRelativeOffset>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.error {
            None
        } else {
            match self.get_next_instr() {
                Ok(r) => match r {
                    Some(i) => Some(Ok(i)),
                    None => None,
                },
                Err(e) => {
                    self.error = true;
                    Some(Err(e))
                }
            }
        }
    }
}
