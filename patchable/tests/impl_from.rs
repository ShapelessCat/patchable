use patchable::{Patch, Patchable, patchable_model};

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct Inner {
    value: i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct Outer<InnerType> {
    #[patchable]
    inner: InnerType,
    extra: u32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct TupleOuter<InnerType>(#[patchable] InnerType, u32);

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct UnitOuter;

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct SkipOuter {
    value: i32,
    #[patchable(skip)]
    untouched: u32,
}

#[test]
fn test_from_struct_to_patch() {
    let original = Outer {
        inner: Inner { value: 42 },
        extra: 7,
    };

    let patch: <Outer<Inner> as Patchable>::Patch = original.clone().into();
    let mut target = Outer {
        inner: Inner { value: 0 },
        extra: 0,
    };

    target.patch(patch);
    assert_eq!(target, original);
}

#[test]
fn test_from_tuple_struct_to_patch() {
    let original = TupleOuter(Inner { value: 42 }, 7);
    let patch: <TupleOuter<Inner> as Patchable>::Patch = original.clone().into();
    let mut target = TupleOuter(Inner { value: 0 }, 0);

    target.patch(patch);
    assert_eq!(target, original);
}

#[test]
fn test_from_unit_struct_to_patch() {
    let patch: <UnitOuter as Patchable>::Patch = UnitOuter.into();
    let mut target = UnitOuter;

    target.patch(patch);
    assert_eq!(target, UnitOuter);
}

#[test]
fn test_from_patch_respects_skipped_fields() {
    let original = SkipOuter {
        value: 10,
        untouched: 7,
    };
    let patch: <SkipOuter as Patchable>::Patch = original.into();
    let mut target = SkipOuter {
        value: 0,
        untouched: 99,
    };

    target.patch(patch);
    assert_eq!(target.value, 10);
    assert_eq!(target.untouched, 99);
}
