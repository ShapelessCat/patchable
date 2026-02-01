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
