use patchable::patchable_model;

#[patchable_model]
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
struct Inner {
    x: i32,
}

#[patchable_model]
#[derive(Debug, Clone, PartialEq)]
struct Wrapper {
    #[patchable]
    inner: Inner,
    #[patchable]
    opt: Option<Inner>,
    #[patchable]
    boxed: Box<Inner>,
    #[patchable]
    vec: Vec<Inner>,
}

#[cfg(feature = "impl_from")]
#[test]
fn test_impl_from_conversion() {
    let original = Wrapper {
        inner: Inner { x: 1 },
        opt: Some(Inner { x: 2 }),
        boxed: Box::new(Inner { x: 3 }),
        vec: vec![Inner { x: 4 }],
    };

    let patch: <Wrapper as patchable::Patchable>::Patch = original.clone().into();

    // Check inner
    assert_eq!(patch.inner.x, 1);

    // Check Option
    // Option<Inner::Patch>
    assert!(patch.opt.is_some());
    assert_eq!(patch.opt.unwrap().x, 2);

    // Check Box
    // Box<Inner::Patch>
    assert_eq!(patch.boxed.x, 3);

    // Check Vec
    // Vec<Inner> - Vec generic param T matches T::Patch only if T is replaced?
    // Wait, Vec<T> patch is Vec<T>. So it's just a clone.
    assert_eq!(patch.vec.len(), 1);
    assert_eq!(patch.vec[0].x, 4);
}
