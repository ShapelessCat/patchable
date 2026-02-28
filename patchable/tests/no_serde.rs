use patchable::patchable_model;

const fn plus_one(x: i32) -> i32 {
    x + 1
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct PlainInner {
    value: i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct PlainOuter<T> {
    #[patchable]
    inner: T,
    version: u32,
}

#[derive(Clone, Debug, PartialEq, patchable::Patchable, patchable::Patch)]
struct DeriveOnlyStruct {
    value: i32,
    #[patchable(skip)]
    sticky: u32,
}

#[patchable_model]
#[derive(Clone, Debug)]
struct AllSkipped {
    #[patchable(skip)]
    marker: fn(i32) -> i32,
}

#[patchable_model]
#[derive(Clone, Debug, PartialEq)]
struct FieldWithNonPatchableAttrBeforeSkip {
    value: i32,
    #[allow(dead_code)]
    #[patchable(skip)]
    sticky: u32,
}

#[test]
fn test_patchable_model_and_derive_generate_patch_types_without_serde() {
    fn assert_patchable<T: patchable::Patchable + patchable::Patch>() {}

    assert_patchable::<PlainInner>();
    assert_patchable::<PlainOuter<PlainInner>>();
    assert_patchable::<DeriveOnlyStruct>();
    assert_patchable::<AllSkipped>();
}

#[test]
fn test_patch_methods_are_generated_without_serde() {
    let _: fn(
        &mut PlainOuter<PlainInner>,
        <PlainOuter<PlainInner> as patchable::Patchable>::Patch,
    ) = <PlainOuter<PlainInner> as patchable::Patch>::patch;

    let _: fn(&mut DeriveOnlyStruct, <DeriveOnlyStruct as patchable::Patchable>::Patch) =
        <DeriveOnlyStruct as patchable::Patch>::patch;

    let _: fn(&mut AllSkipped, <AllSkipped as patchable::Patchable>::Patch) =
        <AllSkipped as patchable::Patch>::patch;

    let outer_patch_name =
        std::any::type_name::<<PlainOuter<PlainInner> as patchable::Patchable>::Patch>();
    let derive_patch_name =
        std::any::type_name::<<DeriveOnlyStruct as patchable::Patchable>::Patch>();
    assert!(outer_patch_name.contains("PlainOuter"));
    assert!(derive_patch_name.contains("DeriveOnlyStruct"));

    let value = AllSkipped { marker: plus_one };
    assert_eq!((value.marker)(1), 2);
}

#[test]
fn test_patchable_skip_works_with_non_patchable_field_attribute() {
    let _: fn(
        &mut FieldWithNonPatchableAttrBeforeSkip,
        <FieldWithNonPatchableAttrBeforeSkip as patchable::Patchable>::Patch,
    ) = <FieldWithNonPatchableAttrBeforeSkip as patchable::Patch>::patch;
}
