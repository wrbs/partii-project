use super::types::*;
use crate::Opcode;
use std::iter::Peekable;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstructionParseErrorReason {
    #[error("unexpected end of stream")]
    UnexpectedEnd,

    #[error("negative label of {0}")]
    NegativeLabel(i32),

    #[error("unknown opcode: {0}")]
    BadOpcode(i32),
}

#[derive(Debug, Error)]
#[error("Instruction parse error at {current_position}: {reason}")]
pub struct InstructionParseError {
    pub reason: InstructionParseErrorReason,
    pub current_position: usize,
    pub parsed_so_far: Vec<Instruction<BytecodeRelativeOffset>>,
}

// default reuslt type in this file
type Result<T, E = InstructionParseErrorReason> = std::result::Result<T, E>;

pub fn parse_instructions_from_code_slice(
    code: &[i32],
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, InstructionParseError> {
    parse_instructions(code.iter().copied())
}

pub fn parse_instructions<I: Iterator<Item = i32>>(
    iterator: I,
) -> Result<Vec<Instruction<BytecodeRelativeOffset>>, InstructionParseError> {
    let mut context = ParseContext::new(iterator);
    let mut result = Vec::new();

    match parse_instructions_body(&mut context, &mut result) {
        Ok(()) => Ok(result),
        Err(reason) => Err(InstructionParseError {
            reason,
            current_position: context.position(),
            parsed_so_far: result,
        }),
    }
}

fn parse_instructions_body<I: Iterator<Item = i32>>(
    context: &mut ParseContext<I>,
    result: &mut Vec<Instruction<BytecodeRelativeOffset>>,
) -> Result<()> {
    while !context.at_end() {
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

        result.push(Instruction::LabelDef(BytecodeRelativeOffset(
            context.position(),
        )));

        // Every bytecode instruction has at least one simplified instruction pushed so we simplify
        // things in most cases by pushing this at the end
        let to_push = match context.opcode()? {
            Opcode::Acc0 => Instruction::Acc(0),
            Opcode::Acc1 => Instruction::Acc(1),
            Opcode::Acc2 => Instruction::Acc(2),
            Opcode::Acc3 => Instruction::Acc(3),
            Opcode::Acc4 => Instruction::Acc(4),
            Opcode::Acc5 => Instruction::Acc(5),
            Opcode::Acc6 => Instruction::Acc(6),
            Opcode::Acc7 => Instruction::Acc(7),
            Opcode::Acc => Instruction::Acc(context.u32()?),

            Opcode::PushAcc0 => {
                result.push(Instruction::Push);
                Instruction::Acc(0)
            }
            Opcode::PushAcc1 => {
                result.push(Instruction::Push);
                Instruction::Acc(1)
            }
            Opcode::PushAcc2 => {
                result.push(Instruction::Push);
                Instruction::Acc(2)
            }
            Opcode::PushAcc3 => {
                result.push(Instruction::Push);
                Instruction::Acc(3)
            }
            Opcode::PushAcc4 => {
                result.push(Instruction::Push);
                Instruction::Acc(4)
            }
            Opcode::PushAcc5 => {
                result.push(Instruction::Push);
                Instruction::Acc(5)
            }
            Opcode::PushAcc6 => {
                result.push(Instruction::Push);
                Instruction::Acc(6)
            }
            Opcode::PushAcc7 => {
                result.push(Instruction::Push);
                Instruction::Acc(7)
            }
            Opcode::PushAcc => {
                result.push(Instruction::Push);
                Instruction::Acc(context.u32()?)
            }

            Opcode::EnvAcc1 => Instruction::EnvAcc(1),
            Opcode::EnvAcc2 => Instruction::EnvAcc(2),
            Opcode::EnvAcc3 => Instruction::EnvAcc(3),
            Opcode::EnvAcc4 => Instruction::EnvAcc(4),
            Opcode::EnvAcc => Instruction::EnvAcc(context.u32()?),

            Opcode::PushEnvAcc1 => {
                result.push(Instruction::Push);
                Instruction::EnvAcc(1)
            }
            Opcode::PushEnvAcc2 => {
                result.push(Instruction::Push);
                Instruction::EnvAcc(2)
            }
            Opcode::PushEnvAcc3 => {
                result.push(Instruction::Push);
                Instruction::EnvAcc(3)
            }
            Opcode::PushEnvAcc4 => {
                result.push(Instruction::Push);
                Instruction::EnvAcc(4)
            }
            Opcode::PushEnvAcc => {
                result.push(Instruction::Push);
                Instruction::EnvAcc(context.u32()?)
            }

            Opcode::Push => Instruction::Push,
            Opcode::Pop => Instruction::Pop(context.u32()?),
            Opcode::Assign => Instruction::Assign(context.u32()?),
            Opcode::PushRetAddr => Instruction::PushRetAddr(context.label()?),

            Opcode::Apply1 => Instruction::Apply1,
            Opcode::Apply2 => Instruction::Apply2,
            Opcode::Apply3 => Instruction::Apply3,
            Opcode::Apply => Instruction::Apply(context.u32()?),

            Opcode::AppTerm1 => Instruction::ApplyTerm(1, context.u32()?),
            Opcode::AppTerm2 => Instruction::ApplyTerm(2, context.u32()?),
            Opcode::AppTerm3 => Instruction::ApplyTerm(3, context.u32()?),
            Opcode::AppTerm => Instruction::ApplyTerm(context.u32()?, context.u32()?),

            Opcode::Return => Instruction::Return(context.u32()?),
            Opcode::Restart => Instruction::Restart,
            Opcode::Grab => {
                let pos = context.position();
                let prev_restart = pos - 2;
                let n = context.u32()?;
                Instruction::Grab(BytecodeRelativeOffset(prev_restart), n)
            }

            Opcode::Closure => {
                let n = context.u32()?;
                let label = context.label()?;
                Instruction::Closure(label, n)
            }
            Opcode::ClosureRec => {
                let length = context.u32()?;
                let n = context.u32()?;
                let pos = context.position();
                let data = context.get_label_list(length as usize, pos)?;
                Instruction::ClosureRec(data, n)
            }

            Opcode::OffsetClosure0 => Instruction::OffsetClosure(0),
            Opcode::OffsetClosure2 => Instruction::OffsetClosure(2),
            Opcode::OffsetClosureM2 => Instruction::OffsetClosure(-2),
            Opcode::OffsetClosure => Instruction::OffsetClosure(context.i32()?),

            Opcode::PushOffsetClosure0 => {
                result.push(Instruction::Push);
                Instruction::OffsetClosure(0)
            }
            Opcode::PushOffsetClosure2 => {
                result.push(Instruction::Push);
                Instruction::OffsetClosure(2)
            }
            Opcode::PushOffsetClosureM2 => {
                result.push(Instruction::Push);
                Instruction::OffsetClosure(-2)
            }
            Opcode::PushOffsetClosure => {
                result.push(Instruction::Push);
                Instruction::OffsetClosure(context.i32()?)
            }

            Opcode::GetGlobal => Instruction::GetGlobal(context.u32()?),
            Opcode::PushGetGlobal => {
                result.push(Instruction::Push);
                Instruction::GetGlobal(context.u32()?)
            }
            Opcode::GetGlobalField => {
                result.push(Instruction::GetGlobal(context.u32()?));
                Instruction::GetField(context.u32()?)
            }
            Opcode::PushGetGlobalField => {
                result.push(Instruction::Push);
                result.push(Instruction::GetGlobal(context.u32()?));
                Instruction::GetField(context.u32()?)
            }

            Opcode::SetGlobal => Instruction::SetGlobal(context.u32()?),

            // todo: const, makeblock
            Opcode::GetField0 => Instruction::GetField(0),
            Opcode::GetField1 => Instruction::GetField(1),
            Opcode::GetField2 => Instruction::GetField(2),
            Opcode::GetField3 => Instruction::GetField(3),
            Opcode::GetField => Instruction::GetField(context.u32()?),

            Opcode::SetField0 => Instruction::SetField(0),
            Opcode::SetField1 => Instruction::SetField(1),
            Opcode::SetField2 => Instruction::SetField(2),
            Opcode::SetField3 => Instruction::SetField(3),
            Opcode::SetField => Instruction::SetField(context.u32()?),

            Opcode::GetFloatField => Instruction::GetFloatField(context.u32()?),
            Opcode::SetFloatField => Instruction::SetFloatField(context.u32()?),

            Opcode::VecTLength => Instruction::VecTLength,
            Opcode::GetVecTItem => Instruction::GetVecTItem,
            Opcode::SetVecTItem => Instruction::SetVecTItem,

            Opcode::GetStringChar => Instruction::GetBytesChar,
            Opcode::GetBytesChar => Instruction::GetBytesChar,
            Opcode::SetBytesChar => Instruction::SetBytesChar,

            Opcode::Branch => Instruction::Branch(context.label()?),
            Opcode::BranchIf => Instruction::BranchIf(context.label()?),
            Opcode::BranchIfNot => Instruction::BranchIfNot(context.label()?),

            Opcode::Switch => {
                let n = context.u32()?;

                let pos = context.position();
                let ints = context.get_label_list((n & 0xFFFF) as usize, pos)?;
                let tags = context.get_label_list((n >> 16) as usize, pos)?;

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

            Opcode::BEq => Instruction::BranchCmp(Comp::Eq, context.i32()?, context.label()?),
            Opcode::BNeq => Instruction::BranchCmp(Comp::Ne, context.i32()?, context.label()?),
            Opcode::BLtInt => Instruction::BranchCmp(Comp::Lt, context.i32()?, context.label()?),
            Opcode::BLeInt => Instruction::BranchCmp(Comp::Le, context.i32()?, context.label()?),
            Opcode::BGtInt => Instruction::BranchCmp(Comp::Gt, context.i32()?, context.label()?),
            Opcode::BGeInt => Instruction::BranchCmp(Comp::Ge, context.i32()?, context.label()?),
            Opcode::BULtInt => Instruction::BranchCmp(Comp::ULt, context.i32()?, context.label()?),
            Opcode::BUGeInt => Instruction::BranchCmp(Comp::UGe, context.i32()?, context.label()?),

            Opcode::IsInt => Instruction::IsInt,

            Opcode::OffsetInt => Instruction::OffsetInt(context.i32()?),
            Opcode::OffsetRef => Instruction::OffsetRef(context.i32()?),

            Opcode::GetMethod => Instruction::GetMethod,
            Opcode::GetPubMet => {
                let tag = context.i32()?;
                let _cache = context.u32()?;
                result.push(Instruction::SetupForPubMet(tag));
                Instruction::GetDynMet
            }

            Opcode::GetDynMet => Instruction::GetDynMet,

            Opcode::CCall1 => Instruction::CCall1(context.u32()?),
            Opcode::CCall2 => Instruction::CCall2(context.u32()?),
            Opcode::CCall3 => Instruction::CCall3(context.u32()?),
            Opcode::CCall4 => Instruction::CCall4(context.u32()?),
            Opcode::CCall5 => Instruction::CCall5(context.u32()?),
            Opcode::CCallN => Instruction::CCallN(context.u32()?, context.u32()?),

            Opcode::Raise => Instruction::Raise(RaiseKind::Regular),
            Opcode::ReRaise => Instruction::Raise(RaiseKind::ReRaise),
            Opcode::RaiseNoTrace => Instruction::Raise(RaiseKind::NoTrace),

            Opcode::PopTrap => Instruction::PopTrap,
            Opcode::PushTrap => Instruction::PushTrap(context.label()?),

            Opcode::CheckSignals => Instruction::CheckSignals,

            Opcode::Atom => Instruction::MakeBlock(0, context.u8()?),
            Opcode::Atom0 => Instruction::MakeBlock(0, 0),

            Opcode::PushAtom0 => {
                result.push(Instruction::Push);
                Instruction::MakeBlock(0, 0)
            }
            Opcode::PushAtom => {
                result.push(Instruction::Push);
                Instruction::MakeBlock(0, context.u8()?)
            }

            Opcode::MakeBlock1 => Instruction::MakeBlock(1, context.u8()?),
            Opcode::MakeBlock2 => Instruction::MakeBlock(2, context.u8()?),
            Opcode::MakeBlock3 => Instruction::MakeBlock(3, context.u8()?),
            Opcode::MakeBlock => Instruction::MakeBlock(context.u32()?, context.u8()?),
            Opcode::MakeFloatBlock => Instruction::MakeFloatBlock(context.u32()?),

            Opcode::Const0 => Instruction::Const(0),
            Opcode::Const1 => Instruction::Const(1),
            Opcode::Const2 => Instruction::Const(2),
            Opcode::Const3 => Instruction::Const(3),
            Opcode::ConstInt => Instruction::Const(context.i32()?),

            Opcode::PushConst0 => {
                result.push(Instruction::Push);
                Instruction::Const(0)
            }
            Opcode::PushConst1 => {
                result.push(Instruction::Push);
                Instruction::Const(1)
            }
            Opcode::PushConst2 => {
                result.push(Instruction::Push);
                Instruction::Const(2)
            }
            Opcode::PushConst3 => {
                result.push(Instruction::Push);
                Instruction::Const(3)
            }
            Opcode::PushConstInt => {
                result.push(Instruction::Push);
                Instruction::Const(context.i32()?)
            }

            Opcode::Stop => Instruction::Stop,
            Opcode::Break => Instruction::Break,
            Opcode::Event => Instruction::Event,
        };

        result.push(to_push);
    }

    Ok(())
}

struct ParseContext<I: Iterator<Item = i32>> {
    iter: Peekable<I>,
    position: usize,
}

impl<I: Iterator<Item = i32>> ParseContext<I> {
    fn new(iterator: I) -> ParseContext<I> {
        ParseContext {
            iter: iterator.peekable(),
            position: 0,
        }
    }

    fn position(&self) -> usize {
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
            None => Err(InstructionParseErrorReason::UnexpectedEnd),
        }
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(self.i32()? as u32)
    }

    fn opcode(&mut self) -> Result<Opcode> {
        let v = self.i32()?;
        match Opcode::from_i32(v) {
            Some(x) => Ok(x),
            None => Err(InstructionParseErrorReason::BadOpcode(v)),
        }
    }

    fn label_at(&mut self, position: usize) -> Result<BytecodeRelativeOffset> {
        let rel = self.i32()?;
        let location = position as i32 + rel;
        if location < 0 {
            return Err(InstructionParseErrorReason::NegativeLabel(location));
        }
        Ok(BytecodeRelativeOffset(location as usize))
    }

    fn label(&mut self) -> Result<BytecodeRelativeOffset> {
        self.label_at(self.position())
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
}
