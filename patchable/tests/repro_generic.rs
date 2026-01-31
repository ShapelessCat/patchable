#[cfg(test)]
mod test {
    use patchable::{Patch, Patchable, patchable_model};
    use serde::Deserialize;

    #[patchable_model]
    #[derive(Clone, Debug, PartialEq, Default, Deserialize)]
    struct Inner {
        val: i32,
    }

    #[patchable_model]
    #[derive(Clone, Debug, PartialEq)]
    struct Container {
        #[patchable]
        inner_box: Box<Inner>,
        #[patchable]
        inner_opt: Option<Inner>,
        #[patchable]
        inner_vec: Vec<Inner>,
    }

    #[test]
    fn test_generic_wrappers() {
        let original = Container {
            inner_box: Box::new(Inner { val: 1 }),
            inner_opt: Some(Inner { val: 1 }),
            inner_vec: vec![Inner { val: 1 }],
        };

        // Construct patch via JSON to avoid knowing exact struct names
        let patch_json = r#"{
            "inner_box": { "val": 2 },
            "inner_opt": { "val": 2 },
            "inner_vec": [{ "val": 2 }]
        }"#;

        let patch: <Container as Patchable>::Patch =
            serde_json::from_str(patch_json).expect("Failed to parse patch");

        let mut target = original.clone();
        target.patch(patch);

        assert_eq!(target.inner_box.val, 2);
        assert_eq!(target.inner_opt.unwrap().val, 2);
        assert_eq!(target.inner_vec[0].val, 2);
    }
}
