use std::fmt;

pub const NUM_OPERATIONS: usize = 149;

// All opcodes
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Opcode {
    Acc0 = 0,
    Acc1 = 1,
    Acc2 = 2,
    Acc3 = 3,
    Acc4 = 4,
    Acc5 = 5,
    Acc6 = 6,
    Acc7 = 7,
    Acc = 8,
    Push = 9,
    PushAcc0 = 10,
    PushAcc1 = 11,
    PushAcc2 = 12,
    PushAcc3 = 13,
    PushAcc4 = 14,
    PushAcc5 = 15,
    PushAcc6 = 16,
    PushAcc7 = 17,
    PushAcc = 18,
    Pop = 19,
    Assign = 20,
    EnvAcc1 = 21,
    EnvAcc2 = 22,
    EnvAcc3 = 23,
    EnvAcc4 = 24,
    EnvAcc = 25,
    PushEnvAcc1 = 26,
    PushEnvAcc2 = 27,
    PushEnvAcc3 = 28,
    PushEnvAcc4 = 29,
    PushEnvAcc = 30,
    PushRetAddr = 31,
    Apply = 32,
    Apply1 = 33,
    Apply2 = 34,
    Apply3 = 35,
    AppTerm = 36,
    AppTerm1 = 37,
    AppTerm2 = 38,
    AppTerm3 = 39,
    Return = 40,
    Restart = 41,
    Grab = 42,
    Closure = 43,
    ClosureRec = 44,
    OffsetClosureM2 = 45,
    OffsetClosure0 = 46,
    OffsetClosure2 = 47,
    OffsetClosure = 48,
    PushOffsetClosureM2 = 49,
    PushOffsetClosure0 = 50,
    PushOffsetClosure2 = 51,
    PushOffsetClosure = 52,
    GetGlobal = 53,
    PushGetGlobal = 54,
    GetGlobalField = 55,
    PushGetGlobalField = 56,
    SetGlobal = 57,
    Atom0 = 58,
    Atom = 59,
    PushAtom0 = 60,
    PushAtom = 61,
    MakeBlock = 62,
    MakeBlock1 = 63,
    MakeBlock2 = 64,
    MakeBlock3 = 65,
    MakeFloatBlock = 66,
    GetField0 = 67,
    GetField1 = 68,
    GetField2 = 69,
    GetField3 = 70,
    GetField = 71,
    GetFloatField = 72,
    SetField0 = 73,
    SetField1 = 74,
    SetField2 = 75,
    SetField3 = 76,
    SetField = 77,
    SetFloatField = 78,
    VecTLength = 79,
    GetVecTItem = 80,
    SetVecTItem = 81,
    GetBytesChar = 82,
    SetBytesChar = 83,
    Branch = 84,
    BranchIf = 85,
    BranchIfNot = 86,
    Switch = 87,
    BoolNot = 88,
    PushTrap = 89,
    PopTrap = 90,
    Raise = 91,
    CheckSignals = 92,
    CCall1 = 93,
    CCall2 = 94,
    CCall3 = 95,
    CCall4 = 96,
    CCall5 = 97,
    CCallN = 98,
    Const0 = 99,
    Const1 = 100,
    Const2 = 101,
    Const3 = 102,
    ConstInt = 103,
    PushConst0 = 104,
    PushConst1 = 105,
    PushConst2 = 106,
    PushConst3 = 107,
    PushConstInt = 108,
    NegInt = 109,
    AddInt = 110,
    SubInt = 111,
    MulInt = 112,
    DivInt = 113,
    ModInt = 114,
    AndInt = 115,
    OrInt = 116,
    XorInt = 117,
    LslInt = 118,
    LsrInt = 119,
    AsrInt = 120,
    Eq = 121,
    Neq = 122,
    LtInt = 123,
    LeInt = 124,
    GtInt = 125,
    GeInt = 126,
    OffsetInt = 127,
    OffsetRef = 128,
    IsInt = 129,
    GetMethod = 130,
    BEq = 131,
    BNeq = 132,
    BLtInt = 133,
    BLeInt = 134,
    BGtInt = 135,
    BGeInt = 136,
    ULtInt = 137,
    UGeInt = 138,
    BULtInt = 139,
    BUGeInt = 140,
    GetPubMet = 141,
    GetDynMet = 142,
    Stop = 143,
    Event = 144,
    Break = 145,
    ReRaise = 146,
    RaiseNoTrace = 147,
    GetStringChar = 148,
}

// Opcode metadata

// This is the shape of the opcode - the number of parameters it takes
// This data is mainly useful for disassembly and decoding of instructions

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Shape {
    Nothing,
    Uint,
    Sint,
    UintUint,
    Disp,
    UintDisp,
    SintDisp,
    GetGlobal,
    GetGlobalUint,
    SetGlobal,
    Primitive,
    UIntPrimitive,
    Switch,
    ClosureRec,
    PubMet,
}

// (opcode, name, shape)
const OPCODE_METADATA: &[(Opcode, &str, Shape)] = &[
    (Opcode::Acc0, "ACC0", Shape::Nothing),
    (Opcode::Acc1, "ACC1", Shape::Nothing),
    (Opcode::Acc2, "ACC2", Shape::Nothing),
    (Opcode::Acc3, "ACC3", Shape::Nothing),
    (Opcode::Acc4, "ACC4", Shape::Nothing),
    (Opcode::Acc5, "ACC5", Shape::Nothing),
    (Opcode::Acc6, "ACC6", Shape::Nothing),
    (Opcode::Acc7, "ACC7", Shape::Nothing),
    (Opcode::Acc, "ACC", Shape::Nothing),
    (Opcode::Push, "PUSH", Shape::Nothing),
    (Opcode::PushAcc0, "PUSHACC0", Shape::Nothing),
    (Opcode::PushAcc1, "PUSHACC1", Shape::Nothing),
    (Opcode::PushAcc2, "PUSHACC2", Shape::Nothing),
    (Opcode::PushAcc3, "PUSHACC3", Shape::Nothing),
    (Opcode::PushAcc4, "PUSHACC4", Shape::Nothing),
    (Opcode::PushAcc5, "PUSHACC5", Shape::Nothing),
    (Opcode::PushAcc6, "PUSHACC6", Shape::Nothing),
    (Opcode::PushAcc7, "PUSHACC7", Shape::Nothing),
    (Opcode::PushAcc, "PUSHACC", Shape::Nothing),
    (Opcode::Pop, "POP", Shape::Nothing),
    (Opcode::Assign, "ASSIGN", Shape::Nothing),
    (Opcode::EnvAcc1, "ENVACC1", Shape::Nothing),
    (Opcode::EnvAcc2, "ENVACC2", Shape::Nothing),
    (Opcode::EnvAcc3, "ENVACC3", Shape::Nothing),
    (Opcode::EnvAcc4, "ENVACC4", Shape::Nothing),
    (Opcode::EnvAcc, "ENVACC", Shape::Nothing),
    (Opcode::PushEnvAcc1, "PUSHENVACC1", Shape::Nothing),
    (Opcode::PushEnvAcc2, "PUSHENVACC2", Shape::Nothing),
    (Opcode::PushEnvAcc3, "PUSHENVACC3", Shape::Nothing),
    (Opcode::PushEnvAcc4, "PUSHENVACC4", Shape::Nothing),
    (Opcode::PushEnvAcc, "PUSHENVACC", Shape::Nothing),
    (Opcode::PushRetAddr, "PUSH_RETADDR", Shape::Nothing),
    (Opcode::Apply, "APPLY", Shape::Nothing),
    (Opcode::Apply1, "APPLY1", Shape::Nothing),
    (Opcode::Apply2, "APPLY2", Shape::Nothing),
    (Opcode::Apply3, "APPLY3", Shape::Nothing),
    (Opcode::AppTerm, "APPTERM", Shape::Nothing),
    (Opcode::AppTerm1, "APPTERM1", Shape::Nothing),
    (Opcode::AppTerm2, "APPTERM2", Shape::Nothing),
    (Opcode::AppTerm3, "APPTERM3", Shape::Nothing),
    (Opcode::Return, "RETURN", Shape::Nothing),
    (Opcode::Restart, "RESTART", Shape::Nothing),
    (Opcode::Grab, "GRAB", Shape::Nothing),
    (Opcode::Closure, "CLOSURE", Shape::Nothing),
    (Opcode::ClosureRec, "CLOSUREREC", Shape::Nothing),
    (Opcode::OffsetClosureM2, "OFFSETCLOSUREM2", Shape::Nothing),
    (Opcode::OffsetClosure0, "OFFSETCLOSURE0", Shape::Nothing),
    (Opcode::OffsetClosure2, "OFFSETCLOSURE2", Shape::Nothing),
    (Opcode::OffsetClosure, "OFFSETCLOSURE", Shape::Nothing),
    (
        Opcode::PushOffsetClosureM2,
        "PUSHOFFSETCLOSUREM2",
        Shape::Nothing,
    ),
    (
        Opcode::PushOffsetClosure0,
        "PUSHOFFSETCLOSURE0",
        Shape::Nothing,
    ),
    (
        Opcode::PushOffsetClosure2,
        "PUSHOFFSETCLOSURE2",
        Shape::Nothing,
    ),
    (
        Opcode::PushOffsetClosure,
        "PUSHOFFSETCLOSURE",
        Shape::Nothing,
    ),
    (Opcode::GetGlobal, "GETGLOBAL", Shape::Nothing),
    (Opcode::PushGetGlobal, "PUSHGETGLOBAL", Shape::Nothing),
    (Opcode::GetGlobalField, "GETGLOBALFIELD", Shape::Nothing),
    (
        Opcode::PushGetGlobalField,
        "PUSHGETGLOBALFIELD",
        Shape::Nothing,
    ),
    (Opcode::SetGlobal, "SETGLOBAL", Shape::Nothing),
    (Opcode::Atom0, "ATOM0", Shape::Nothing),
    (Opcode::Atom, "ATOM", Shape::Nothing),
    (Opcode::PushAtom0, "PUSHATOM0", Shape::Nothing),
    (Opcode::PushAtom, "PUSHATOM", Shape::Nothing),
    (Opcode::MakeBlock, "MAKEBLOCK", Shape::Nothing),
    (Opcode::MakeBlock1, "MAKEBLOCK1", Shape::Nothing),
    (Opcode::MakeBlock2, "MAKEBLOCK2", Shape::Nothing),
    (Opcode::MakeBlock3, "MAKEBLOCK3", Shape::Nothing),
    (Opcode::MakeFloatBlock, "MAKEFLOATBLOCK", Shape::Nothing),
    (Opcode::GetField0, "GETFIELD0", Shape::Nothing),
    (Opcode::GetField1, "GETFIELD1", Shape::Nothing),
    (Opcode::GetField2, "GETFIELD2", Shape::Nothing),
    (Opcode::GetField3, "GETFIELD3", Shape::Nothing),
    (Opcode::GetField, "GETFIELD", Shape::Nothing),
    (Opcode::GetFloatField, "GETFLOATFIELD", Shape::Nothing),
    (Opcode::SetField0, "SETFIELD0", Shape::Nothing),
    (Opcode::SetField1, "SETFIELD1", Shape::Nothing),
    (Opcode::SetField2, "SETFIELD2", Shape::Nothing),
    (Opcode::SetField3, "SETFIELD3", Shape::Nothing),
    (Opcode::SetField, "SETFIELD", Shape::Nothing),
    (Opcode::SetFloatField, "SETFLOATFIELD", Shape::Nothing),
    (Opcode::VecTLength, "VECTLENGTH", Shape::Nothing),
    (Opcode::GetVecTItem, "GETVECTITEM", Shape::Nothing),
    (Opcode::SetVecTItem, "SETVECTITEM", Shape::Nothing),
    (Opcode::GetBytesChar, "GETBYTESCHAR", Shape::Nothing),
    (Opcode::SetBytesChar, "SETBYTESCHAR", Shape::Nothing),
    (Opcode::Branch, "BRANCH", Shape::Nothing),
    (Opcode::BranchIf, "BRANCHIF", Shape::Nothing),
    (Opcode::BranchIfNot, "BRANCHIFNOT", Shape::Nothing),
    (Opcode::Switch, "SWITCH", Shape::Nothing),
    (Opcode::BoolNot, "BOOLNOT", Shape::Nothing),
    (Opcode::PushTrap, "PUSHTRAP", Shape::Nothing),
    (Opcode::PopTrap, "POPTRAP", Shape::Nothing),
    (Opcode::Raise, "RAISE", Shape::Nothing),
    (Opcode::CheckSignals, "CHECK_SIGNALS", Shape::Nothing),
    (Opcode::CCall1, "C_CALL1", Shape::Nothing),
    (Opcode::CCall2, "C_CALL2", Shape::Nothing),
    (Opcode::CCall3, "C_CALL3", Shape::Nothing),
    (Opcode::CCall4, "C_CALL4", Shape::Nothing),
    (Opcode::CCall5, "C_CALL5", Shape::Nothing),
    (Opcode::CCallN, "C_CALLN", Shape::Nothing),
    (Opcode::Const0, "CONST0", Shape::Nothing),
    (Opcode::Const1, "CONST1", Shape::Nothing),
    (Opcode::Const2, "CONST2", Shape::Nothing),
    (Opcode::Const3, "CONST3", Shape::Nothing),
    (Opcode::ConstInt, "CONSTINT", Shape::Nothing),
    (Opcode::PushConst0, "PUSHCONST0", Shape::Nothing),
    (Opcode::PushConst1, "PUSHCONST1", Shape::Nothing),
    (Opcode::PushConst2, "PUSHCONST2", Shape::Nothing),
    (Opcode::PushConst3, "PUSHCONST3", Shape::Nothing),
    (Opcode::PushConstInt, "PUSHCONSTINT", Shape::Nothing),
    (Opcode::NegInt, "NEGINT", Shape::Nothing),
    (Opcode::AddInt, "ADDINT", Shape::Nothing),
    (Opcode::SubInt, "SUBINT", Shape::Nothing),
    (Opcode::MulInt, "MULINT", Shape::Nothing),
    (Opcode::DivInt, "DIVINT", Shape::Nothing),
    (Opcode::ModInt, "MODINT", Shape::Nothing),
    (Opcode::AndInt, "ANDINT", Shape::Nothing),
    (Opcode::OrInt, "ORINT", Shape::Nothing),
    (Opcode::XorInt, "XORINT", Shape::Nothing),
    (Opcode::LslInt, "LSLINT", Shape::Nothing),
    (Opcode::LsrInt, "LSRINT", Shape::Nothing),
    (Opcode::AsrInt, "ASRINT", Shape::Nothing),
    (Opcode::Eq, "EQ", Shape::Nothing),
    (Opcode::Neq, "NEQ", Shape::Nothing),
    (Opcode::LtInt, "LTINT", Shape::Nothing),
    (Opcode::LeInt, "LEINT", Shape::Nothing),
    (Opcode::GtInt, "GTINT", Shape::Nothing),
    (Opcode::GeInt, "GEINT", Shape::Nothing),
    (Opcode::OffsetInt, "OFFSETINT", Shape::Nothing),
    (Opcode::OffsetRef, "OFFSETREF", Shape::Nothing),
    (Opcode::IsInt, "ISINT", Shape::Nothing),
    (Opcode::GetMethod, "GETMETHOD", Shape::Nothing),
    (Opcode::BEq, "BEQ", Shape::Nothing),
    (Opcode::BNeq, "BNEQ", Shape::Nothing),
    (Opcode::BLtInt, "BLTINT", Shape::Nothing),
    (Opcode::BLeInt, "BLEINT", Shape::Nothing),
    (Opcode::BGtInt, "BGTINT", Shape::Nothing),
    (Opcode::BGeInt, "BGEINT", Shape::Nothing),
    (Opcode::ULtInt, "ULTINT", Shape::Nothing),
    (Opcode::UGeInt, "UGEINT", Shape::Nothing),
    (Opcode::BULtInt, "BULTINT", Shape::Nothing),
    (Opcode::BUGeInt, "BUGEINT", Shape::Nothing),
    (Opcode::GetPubMet, "GETPUBMET", Shape::Nothing),
    (Opcode::GetDynMet, "GETDYNMET", Shape::Nothing),
    (Opcode::Stop, "STOP", Shape::Nothing),
    (Opcode::Event, "EVENT", Shape::Nothing),
    (Opcode::Break, "BREAK", Shape::Nothing),
    (Opcode::ReRaise, "RERAISE", Shape::Nothing),
    (Opcode::RaiseNoTrace, "RAISE_NOTRACE", Shape::Nothing),
    (Opcode::GetStringChar, "GETSTRINGCHAR", Shape::Nothing),
];

impl Opcode {
    pub fn from_i32(v: i32) -> Option<Opcode> {
        if v < 0 || v >= NUM_OPERATIONS as i32 {
            None
        } else {
            Some(OPCODE_METADATA[v as usize].0)
        }
    }

    pub fn from_u8(v: u8) -> Option<Opcode> {
        Opcode::from_i32(v as i32)
    }

    pub fn ocaml_name(&self) -> &'static str {
        OPCODE_METADATA[*self as usize].1
    }

    pub fn shape(&self) -> Shape {
        OPCODE_METADATA[*self as usize].2
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.ocaml_name())
    }
}
#[cfg(test)]
mod test {
    pub use super::*;

    #[test]
    fn test_of_u8() {
        let max = NUM_OPERATIONS as u8;
        for i in 0..max {
            let opcode = Opcode::from_u8(i).expect("Opcode not parsed");
            assert_eq!(opcode as u8, i);
        }
        for i in (max + 1)..=255 {
            assert_eq!(Opcode::from_u8(i), None);
        }
    }
}
