use strum::{EnumCount, IntoEnumIterator};
use strum_macros::{EnumIter, IntoStaticStr};

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
    OcamlExternSp,
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
pub enum CraneliftPrimitiveFunction {}

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
