use std::io::Read;

use crate::Opcode;
use super::types::*;
use std::iter::Peekable;

pub fn parse_instructions<I: Iterator<Item=i32>>(iterator: I) -> Option<Vec<(usize, Instruction)>> {
    let mut context = ParseContext::new(iterator);
    let mut instructions = Vec::new();

    while !context.at_end() {
        let loc = context.position();
        instructions.push((loc, get_instruction(&mut context)?));
    }

    Some(instructions)
}

fn get_instruction<I: Iterator<Item=i32>>(context: &mut ParseContext<I>) -> Option<Instruction> {
    Some(match context.opcode()? {
        Opcode::Acc0 => Instruction::Acc(0),
        Opcode::Acc1 => Instruction::Acc(1),
        Opcode::Acc2 => Instruction::Acc(2),
        Opcode::Acc3 => Instruction::Acc(3),
        Opcode::Acc4 => Instruction::Acc(4),
        Opcode::Acc5 => Instruction::Acc(5),
        Opcode::Acc6 => Instruction::Acc(6),
        Opcode::Acc7 => Instruction::Acc(7),
        Opcode::Acc => Instruction::Acc(context.i32()?),

        Opcode::PushAcc0 => Instruction::PushAcc(0),
        Opcode::PushAcc1 => Instruction::PushAcc(1),
        Opcode::PushAcc2 => Instruction::PushAcc(2),
        Opcode::PushAcc3 => Instruction::PushAcc(3),
        Opcode::PushAcc4 => Instruction::PushAcc(4),
        Opcode::PushAcc5 => Instruction::PushAcc(5),
        Opcode::PushAcc6 => Instruction::PushAcc(6),
        Opcode::PushAcc7 => Instruction::PushAcc(7),
        Opcode::PushAcc => Instruction::PushAcc(context.i32()?),

        Opcode::EnvAcc1 => Instruction::EnvAcc(1),
        Opcode::EnvAcc2 => Instruction::EnvAcc(2),
        Opcode::EnvAcc3 => Instruction::EnvAcc(3),
        Opcode::EnvAcc4 => Instruction::EnvAcc(4),
        Opcode::EnvAcc => Instruction::EnvAcc(context.i32()?),

        Opcode::PushEnvAcc1 => Instruction::PushEnvAcc(1),
        Opcode::PushEnvAcc2 => Instruction::PushEnvAcc(2),
        Opcode::PushEnvAcc3 => Instruction::PushEnvAcc(3),
        Opcode::PushEnvAcc4 => Instruction::PushEnvAcc(4),
        Opcode::PushEnvAcc => Instruction::PushEnvAcc(context.i32()?),

        Opcode::Push => Instruction::Push,
        Opcode::Pop => Instruction::Pop(context.i32()?),
        Opcode::Assign => Instruction::Assign(context.i32()?),
        Opcode::PushRetAddr => Instruction::PushRetAddr(context.label()?),

        Opcode::Apply1 => Instruction::Apply(1),
        Opcode::Apply2 => Instruction::Apply(2),
        Opcode::Apply3 => Instruction::Apply(3),
        Opcode::Apply => Instruction::Apply(context.i32()?),

        Opcode::AppTerm1 => Instruction::ApplyTerm(1, context.i32()?),
        Opcode::AppTerm2 => Instruction::ApplyTerm(1, context.i32()?),
        Opcode::AppTerm3 => Instruction::ApplyTerm(1, context.i32()?),
        Opcode::AppTerm => Instruction::ApplyTerm(context.i32()?, context.i32()?),

        Opcode::Return => Instruction::Return(context.i32()?),
        Opcode::Restart => Instruction::Restart,
        Opcode::Grab => Instruction::Grab(context.i32()?),

        Opcode::Closure => {
            let n = context.i32()?;
            let label = context.label()?;
            Instruction::Closure(label, n)
        },
        Opcode::ClosureRec => {
            let length = context.i32()?;
            assert!(length > 0);
            let n = context.i32()?;
            let pos = context.position();
            let data = context.get_label_list(length as usize, pos)?;
            Instruction::ClosureRec(data, n)
        },

        Opcode::OffsetClosure0 => Instruction::OffsetClosure(0),
        Opcode::OffsetClosure2 => Instruction::OffsetClosure(2),
        Opcode::OffsetClosureM2 => Instruction::OffsetClosure(-2),
        Opcode::OffsetClosure => Instruction::OffsetClosure(context.i32()?),

        Opcode::PushOffsetClosure0 => Instruction::PushOffsetClosure(0),
        Opcode::PushOffsetClosure2 => Instruction::PushOffsetClosure(2),
        Opcode::PushOffsetClosureM2 => Instruction::PushOffsetClosure(-2),
        Opcode::PushOffsetClosure => Instruction::PushOffsetClosure(context.i32()?),

        Opcode::GetGlobal => Instruction::GetGlobal(context.i32()?),
        Opcode::PushGetGlobal => Instruction::PushGetGlobal(context.i32()?),
        Opcode::GetGlobalField => Instruction::GetGlobalField(context.i32()?, context.i32()?),
        Opcode::PushGetGlobalField => Instruction::PushGetGlobalField(context.i32()?, context.i32()?),

        Opcode::SetGlobal => Instruction::SetGlobal(context.i32()?),

        // todo: const, makeblock

        Opcode::GetField0 => Instruction::GetField(0),
        Opcode::GetField1 => Instruction::GetField(1),
        Opcode::GetField2 => Instruction::GetField(2),
        Opcode::GetField3 => Instruction::GetField(3),
        Opcode::GetField => Instruction::GetField(context.i32()?),

        Opcode::SetField0 => Instruction::SetField(0),
        Opcode::SetField1 => Instruction::SetField(1),
        Opcode::SetField2 => Instruction::SetField(2),
        Opcode::SetField3 => Instruction::SetField(3),
        Opcode::SetField => Instruction::SetField(context.i32()?),

        Opcode::GetFloatField => Instruction::GetFloatField(context.i32()?),
        Opcode::SetFloatField => Instruction::SetFloatField(context.i32()?),

        Opcode::VecTLength => Instruction::VecTLength,
        Opcode::GetVecTItem => Instruction::GetVecTItem,
        Opcode::SetVecTItem => Instruction::SetVecTItem,

        Opcode::GetStringChar => Instruction::GetStringChar,
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

        Opcode::NegInt => Instruction::ArithInt(ArithOp::Neg),
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
            let extra = context.i32()?;
            assert_eq!(extra, 0);
            Instruction::GetPubMet(tag)
        }

        Opcode::GetDynMet => Instruction::GetDynMet,

        Opcode::CCall1 => Instruction::CCall(1, context.primitive()?),
        Opcode::CCall2 => Instruction::CCall(2, context.primitive()?),
        Opcode::CCall3 => Instruction::CCall(3, context.primitive()?),
        Opcode::CCall4 => Instruction::CCall(4, context.primitive()?),
        Opcode::CCall5 => Instruction::CCall(5, context.primitive()?),
        Opcode::CCallN => Instruction::CCall(context.i32()?, context.primitive()?),

        Opcode::Raise => Instruction::Raise(RaiseKind::Regular),
        Opcode::ReRaise => Instruction::Raise(RaiseKind::ReRaise),
        Opcode::RaiseNoTrace => Instruction::Raise(RaiseKind::NoTrace),

        Opcode::PopTrap => Instruction::PopTrap,
        Opcode::PushTrap => Instruction::PushTrap(context.label()?),

        Opcode::CheckSignals => Instruction::CheckSignals,

        Opcode::Atom => Instruction::MakeBlock(0, context.i32()?),
        Opcode::Atom0 => Instruction::MakeBlock(0, 0),

        Opcode::PushAtom0 => Instruction::PushAtom(0),
        Opcode::PushAtom => Instruction::PushAtom(context.i32()?),

        Opcode::MakeBlock1 => Instruction::MakeBlock(1, context.i32()?),
        Opcode::MakeBlock2 => Instruction::MakeBlock(2, context.i32()?),
        Opcode::MakeBlock3 => Instruction::MakeBlock(3, context.i32()?),
        Opcode::MakeBlock => Instruction::MakeBlock(context.i32()?, context.i32()?),
        Opcode::MakeFloatBlock => Instruction::MakeFloatBlock(context.i32()?),

        Opcode::Const0 => Instruction::Const(0),
        Opcode::Const1 => Instruction::Const(1),
        Opcode::Const2 => Instruction::Const(2),
        Opcode::Const3 => Instruction::Const(3),
        Opcode::ConstInt => Instruction::Const(context.i32()?),

        Opcode::PushConst0 => Instruction::PushConst(0),
        Opcode::PushConst1 => Instruction::PushConst(1),
        Opcode::PushConst2 => Instruction::PushConst(2),
        Opcode::PushConst3 => Instruction::PushConst(3),
        Opcode::PushConstInt => Instruction::PushConst(context.i32()?),

        Opcode::Stop => Instruction::Stop,
        Opcode::Break => Instruction::Break,
        Opcode::Event => Instruction::Event,

        _ => return None
    })
}


struct ParseContext<I: Iterator<Item=i32>> {
    iter: Peekable<I>,
    position: usize,
}

impl<I: Iterator<Item=i32>> ParseContext<I> {
    fn new(iterator: I) -> ParseContext<I> {
        ParseContext {
            iter: iterator.peekable(),
            position: 0
        }
    }

    fn at_end(&mut self) -> bool {
        self.iter.peek().is_none()
    }

    fn i32(&mut self) -> Option<i32> {
        let v = self.iter.next()?.into();
        self.position += 1;
        Some(v)
    }

    fn position(&self) -> usize {
        self.position
    }

    fn u32(&mut self) -> Option<u32> {
        self.i32().map(|x| x as u32)
    }

    fn opcode(&mut self) -> Option<Opcode> {
        Opcode::from_i32(self.i32()?)
    }

    fn label_at(&mut self, position: usize) -> Option<usize> {
        let rel = self.i32()?;
        let location = position as i32 + rel;
        assert!(location > 0);
        Some(location as usize)
    }


    fn label(&mut self) -> Option<usize> {
        self.label_at(self.position())
    }

    fn get_label_list(&mut self, count: usize, position: usize) -> Option<Vec<usize>> {
        let mut result = Vec::new();
        for _ in 0..count {
            result.push(self.label_at(position)?);
        }

        return Some(result);
    }

    fn primitive(&mut self) -> Option<usize> {
        self.u32().map(|x| x as usize)
    }
}

