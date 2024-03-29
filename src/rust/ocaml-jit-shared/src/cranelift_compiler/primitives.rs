use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumIter, IntoStaticStr};

pub const MAX_YOUNG_WOSIZE: usize = 256;
pub const CLOSURE_TAG: u8 = 247;
pub const INFIX_TAG: u8 = 249;

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    IntoStaticStr,
    EnumIter,
    strum_macros::EnumCount,
)]
#[strum(serialize_all = "snake_case")]
pub enum CraneliftPrimitiveValue {
    CamlStateAddr,
    CallbackReturnAddr,
    GlobalDataAddr,
    CamlSomethingToDoAddr,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    IntoStaticStr,
    EnumIter,
    strum_macros::EnumCount,
)]
#[strum(serialize_all = "snake_case")]
pub enum CraneliftPrimitiveFunction {
    EmitApplyTrace,
    EmitCCallTrace,
    EmitReturnTrace,
    Apply1,
    Apply2,
    Apply3,
    Apply4,
    Apply5,
    ApplyN,
    CamlAllocSmallDispatch,
    CamlAllocShr,
    CamlInitialize,
    CamlModify,
    MakeBlockTrace,
    CamlRaiseZeroDivide,
    CamlProcessPendingActions,
    CamlRaise,
    JitSupportVectLength,
    JitSupportGetDynMet,
}

pub trait CraneliftPrimitive: Sized {
    type Iter: Iterator<Item = Self>;

    const COUNT: usize;

    fn iter() -> Self::Iter;
}

impl<T> CraneliftPrimitive for T
where
    T: EnumCount + Into<&'static str> + IntoEnumIterator,
{
    type Iter = <Self as IntoEnumIterator>::Iterator;

    const COUNT: usize = <Self as EnumCount>::COUNT;

    fn iter() -> Self::Iter {
        <Self as IntoEnumIterator>::iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamlStateField {
    YoungPtr,
    YoungLimit,
    ExceptionPointer,
    YoungBase,
    YoungStart,
    YoungEnd,
    YoungAllocStart,
    YoungAllocEnd,
    YoungAllocMid,
    YoungTrigger,
    MinorHeapWsz,
    InMinorCollection,
    ExtraHeapResourcesMinor,
    RefTable,
    EpheRefTable,
    CustomTable,
    StackLow,
    StackHigh,
    StackThreshold,
    ExternSp,
    TrapSp,
    TrapBarrier,
    ExternalRaise,
    ExnBucket,
    TopOfStack,
    BottomOfStack,
    LastReturnAddress,
    GcRegs,
    BacktraceActive,
    BacktracePos,
    BacktraceBuffer,
    BacktraceLastExn,
    CompareUnordered,
    RequestedMajorSlice,
    RequestedMinorGc,
    LocalRoots,
    StatMinorWords,
    StatPromotedWords,
    StatMajorWords,
    StatMinorCollections,
    StatMajorCollections,
    StatHeapWsz,
    StatTopHeapWsz,
    StatCompactions,
    StatHeapChunks,
    EventlogStartupTimestamp,
    EventlogStartupPid,
    EventlogPaused,
    EventlogEnabled,
    EventlogOut,
}

impl CamlStateField {
    pub const fn offset(&self) -> i32 {
        *self as i32 * 8
    }
}
