use std::fmt::Debug;

use patchable::{Patch, Patchable, TryPatch, patchable_model};
use serde::{Deserialize, Serialize};

fn identity(x: &i32) -> i32 {
    *x
}

#[patchable_model]
#[derive(Clone, Default, Debug, PartialEq)]
struct FakeMeasurement<T, ClosureType> {
    v: T,
    #[patchable(skip)]
    how: ClosureType,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct MeasurementResult<T>(pub T);

#[patchable_model]
#[derive(Clone, Debug)]
struct ScopedMeasurement<ScopeType, MeasurementType, MeasurementOutput> {
    current_control_level: ScopeType,
    #[patchable]
    inner: MeasurementType,
    current_base: MeasurementResult<MeasurementOutput>,
}

#[test]
fn test_scoped_peek() -> anyhow::Result<()> {
    let fake_measurement: FakeMeasurement<i32, fn(&i32) -> i32> = FakeMeasurement {
        v: 42,
        how: identity,
    };
    let scoped_peek0 = ScopedMeasurement {
        current_control_level: 33u32,
        inner: fake_measurement.clone(),
        current_base: MeasurementResult(20i32),
    };
    let mut scoped_peek1 = ScopedMeasurement {
        current_control_level: 0u32,
        inner: fake_measurement.clone(),
        current_base: MeasurementResult(0i32),
    };
    let state0 = serde_json::to_string(&scoped_peek0)?;
    scoped_peek1.patch(serde_json::from_str(&state0)?);
    let state1 = serde_json::to_string(&scoped_peek0)?;
    assert!(state0 == state1);
    Ok(())
}

#[patchable_model]
#[derive(Clone, Default, Debug)]
struct SimpleStruct {
    val: i32,
}

#[test]
fn test_try_patch_blanket_impl() {
    let mut s = SimpleStruct { val: 10 };
    // The derived patch struct is compatible with serde.
    // We use from_str to create the patch value.
    let patch: <SimpleStruct as Patchable>::Patch = serde_json::from_str(r#"{"val": 20}"#).unwrap();

    // Should always succeed for `Patch` types due to the blanket impl.
    let result = s.try_patch(patch);
    assert!(result.is_ok());
    assert_eq!(s.val, 20);
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
struct TupleStruct(i32, u32);

#[test]
fn test_tuple_struct_patch() {
    let mut s = TupleStruct(1, 2);
    let patch: <TupleStruct as Patchable>::Patch = serde_json::from_str(r#"[10, 20]"#).unwrap();
    s.patch(patch);
    assert_eq!(s, TupleStruct(10, 20));
}

#[patchable_model]
#[derive(Clone, Debug)]
struct TupleStructWithSkippedMiddle<F>(i32, #[patchable(skip)] F, i64);

#[test]
fn test_tuple_struct_skip_keeps_original_field_index() {
    let mut s = TupleStructWithSkippedMiddle(1, identity, 2);
    let patch: <TupleStructWithSkippedMiddle<fn(i32) -> i32> as Patchable>::Patch =
        serde_json::from_str(r#"[10, 20]"#).unwrap();
    s.patch(patch);
    assert_eq!(s.0, 10);
    assert_eq!(s.2, 20);
}

#[patchable_model]
#[derive(Clone, Debug)]
struct TupleStructWithWhereClause<T>(i32, T, i64)
where
    T: From<(u32, u32)>;

#[test]
fn test_tuple_struct_with_where_clause() {
    let mut s = TupleStructWithWhereClause(1, (0, 0), 2);
    let patch: <TupleStructWithWhereClause<(u32, u32)> as Patchable>::Patch =
        serde_json::from_str(r#"[10, [42, 84], 20]"#).unwrap();
    s.patch(patch);
    assert_eq!(s.0, 10);
    assert_eq!(s.1, (42, 84));
    assert_eq!(s.2, 20);
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq, Eq)]
struct UnitStruct;

#[test]
fn test_unit_struct_patch() {
    let mut s = UnitStruct;
    let patch: <UnitStruct as Patchable>::Patch = serde_json::from_str("null").unwrap();
    s.patch(patch);
    assert_eq!(s, UnitStruct);
}

#[patchable_model]
#[derive(Clone, Debug)]
struct SkipSerializingStruct {
    #[patchable(skip)]
    skipped: i32,
    value: i32,
}

#[test]
fn test_skip_serializing_field_is_excluded() {
    let mut s = SkipSerializingStruct {
        skipped: 5,
        value: 10,
    };
    let patch: <SkipSerializingStruct as Patchable>::Patch =
        serde_json::from_str(r#"{"value": 42}"#).unwrap();
    s.patch(patch);
    assert_eq!(s.skipped, 5);
    assert_eq!(s.value, 42);
}
