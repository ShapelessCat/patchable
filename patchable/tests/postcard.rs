use patchable::{Patch, Patchable, TryPatch};
use serde::{Deserialize, Serialize};

mod common;

use common::*;

const POSTCARD_CAPACITY: usize = 512;
type PostcardBytes = heapless::Vec<u8, POSTCARD_CAPACITY>;

fn decode_postcard<T, U>(value: &U) -> T
where
    U: Serialize,
    T: for<'de> Deserialize<'de>,
{
    let bytes = postcard::to_vec::<_, POSTCARD_CAPACITY>(value).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

fn encode_postcard<T>(value: &T) -> PostcardBytes
where
    T: Serialize,
{
    postcard::to_vec::<_, POSTCARD_CAPACITY>(value).unwrap()
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
    let state0 = postcard::to_vec::<_, POSTCARD_CAPACITY>(&scoped_peek0)?;
    scoped_peek1.patch(postcard::from_bytes(&state0)?);
    let state1 = postcard::to_vec::<_, POSTCARD_CAPACITY>(&scoped_peek1)?;
    assert_eq!(state0, state1);
    Ok(())
}

#[test]
fn test_try_patch_blanket_impl() {
    let mut s = SimpleStruct { val: 10 };
    // The derived patch struct is compatible with serde.
    // We deserialize the patch from postcard bytes.
    let patch: <SimpleStruct as Patchable>::Patch = decode_postcard(&SimpleStruct { val: 20 });

    // Should always succeed for `Patch` types due to the blanket impl.
    let result = s.try_patch(patch);
    assert!(result.is_ok());
    assert_eq!(s.val, 20);
}

#[test]
fn test_tuple_struct_patch() {
    let mut s = TupleStruct(1, 2);
    let patch: <TupleStruct as Patchable>::Patch = decode_postcard(&TupleStruct(10, 20));
    s.patch(patch);
    assert_eq!(s, TupleStruct(10, 20));
}

#[test]
fn test_tuple_struct_skip_keeps_original_field_index() {
    let mut s = TupleStructWithSkippedMiddle(1, identity, 2);
    let patch: <TupleStructWithSkippedMiddle<fn(i32) -> i32> as Patchable>::Patch =
        decode_postcard(&(10i32, 20i64));
    s.patch(patch);
    assert_eq!(s.0, 10);
    assert_eq!(s.2, 20);
}

#[test]
fn test_tuple_struct_with_where_clause() {
    let mut s = TupleStructWithWhereClause(1, (0, 0), 2);
    let patch: <TupleStructWithWhereClause<(u32, u32)> as Patchable>::Patch =
        decode_postcard(&(10i32, (42u32, 84u32), 20i64));
    s.patch(patch);
    assert_eq!(s.0, 10);
    assert_eq!(s.1, (42, 84));
    assert_eq!(s.2, 20);
}

#[test]
fn test_unit_struct_patch() {
    let mut s = UnitStruct;
    let patch: <UnitStruct as Patchable>::Patch = decode_postcard(&());
    s.patch(patch);
    assert_eq!(s, UnitStruct);
}

#[test]
fn test_skip_serializing_field_is_excluded() {
    #[derive(Serialize)]
    struct ValueOnly {
        value: i32,
    }

    let mut s = SkipSerializingStruct {
        skipped: 5,
        value: 10,
    };
    let encoded = encode_postcard(&s);
    let expected = encode_postcard(&ValueOnly { value: 10 });
    assert_eq!(encoded, expected);

    let patch: <SkipSerializingStruct as Patchable>::Patch =
        decode_postcard(&ValueOnly { value: 42 });
    s.patch(patch);
    assert_eq!(s.skipped, 5);
    assert_eq!(s.value, 42);
}

#[test]
fn test_direct_derive_does_not_add_serde_skip() {
    #[derive(Serialize)]
    struct FullState {
        hidden: i32,
        shown: i32,
    }

    #[derive(Serialize)]
    struct ShownOnly {
        shown: i32,
    }

    let value = DeriveOnlySkipBehavior {
        hidden: 7,
        shown: 11,
    };
    let encoded = encode_postcard(&value);
    let expected = encode_postcard(&FullState {
        hidden: 7,
        shown: 11,
    });
    assert_eq!(encoded, expected);

    let patch: <DeriveOnlySkipBehavior as Patchable>::Patch =
        decode_postcard(&ShownOnly { shown: 5 });
    let mut target = DeriveOnlySkipBehavior {
        hidden: 99,
        shown: 0,
    };
    target.patch(patch);

    assert_eq!(target.hidden, 99);
    assert_eq!(target.shown, 5);
}

#[test]
fn test_mixed_generic_usage_patches_and_replaces() {
    let mut value = MixedGenericUsage {
        history: [Counter { value: 1 }, Counter { value: 2 }],
        current: Counter { value: 2 },
    };
    let patch: <MixedGenericUsage<Counter, [Counter; 2]> as Patchable>::Patch =
        decode_postcard(&MixedGenericUsage {
            history: [Counter { value: 10 }, Counter { value: 20 }],
            current: Counter { value: 99 },
        });

    value.patch(patch);
    assert_eq!(
        value.history,
        [Counter { value: 10 }, Counter { value: 20 }]
    );
    assert_eq!(value.current, Counter { value: 99 });
}

#[test]
fn test_existing_where_clause_with_trailing_comma() {
    let mut value = ExistingWhereTrailing {
        inner: Counter { value: 1 },
        marker: (),
    };
    let patch: <ExistingWhereTrailing<Counter, ()> as Patchable>::Patch =
        decode_postcard(&ExistingWhereTrailing {
            inner: Counter { value: 5 },
            marker: (),
        });

    value.patch(patch);
    assert_eq!(
        value,
        ExistingWhereTrailing {
            inner: Counter { value: 5 },
            marker: (),
        }
    );
}

#[test]
fn test_existing_where_clause_without_trailing_comma() {
    let mut value = ExistingWhereNoTrailing {
        inner: Counter { value: 3 },
    };
    let patch: <ExistingWhereNoTrailing<Counter> as Patchable>::Patch =
        decode_postcard(&ExistingWhereNoTrailing {
            inner: Counter { value: 8 },
        });

    value.patch(patch);
    assert_eq!(
        value,
        ExistingWhereNoTrailing {
            inner: Counter { value: 8 },
        }
    );
}
