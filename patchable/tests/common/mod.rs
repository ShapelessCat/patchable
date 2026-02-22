use patchable::patchable_model;
use serde::{Deserialize, Serialize};

pub fn identity(x: &i32) -> i32 {
    *x
}

#[patchable_model]
#[derive(Clone, Default, Debug, PartialEq)]
pub struct FakeMeasurement<T, ClosureType> {
    pub v: T,
    #[patchable(skip)]
    pub how: ClosureType,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeasurementResult<T>(pub T);

#[patchable_model]
#[derive(Clone, Debug)]
pub struct ScopedMeasurement<ScopeType, MeasurementType, MeasurementOutput> {
    pub current_control_level: ScopeType,
    #[patchable]
    pub inner: MeasurementType,
    pub current_base: MeasurementResult<MeasurementOutput>,
}

#[patchable_model]
#[derive(Clone, Default, Debug)]
pub struct SimpleStruct {
    pub val: i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleStruct(pub i32, pub u32);

#[patchable_model]
#[derive(Clone, Debug)]
pub struct TupleStructWithSkippedMiddle<F>(pub i32, #[patchable(skip)] pub F, pub i64);

#[patchable_model]
#[derive(Clone, Debug)]
pub struct TupleStructWithWhereClause<T>(pub i32, pub T, pub i64)
where
    T: From<(u32, u32)>;

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitStruct;

#[patchable_model]
#[derive(Clone, Debug)]
pub struct SkipSerializingStruct {
    #[patchable(skip)]
    pub skipped: i32,
    pub value: i32,
}

#[derive(Clone, Debug, Serialize, patchable::Patchable, patchable::Patch)]
pub struct DeriveOnlySkipBehavior {
    #[patchable(skip)]
    pub hidden: i32,
    pub shown: i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Counter {
    pub value: i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixedGenericUsage<T, H> {
    pub history: H,
    #[patchable]
    pub current: T,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingWhereTrailing<T, U>
where
    U: Default,
{
    #[patchable]
    pub inner: T,
    pub marker: U,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExistingWhereNoTrailing<T>
where
    T: Clone,
{
    #[patchable]
    pub inner: T,
}
