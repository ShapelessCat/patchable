use patchable::{Patchable, TryPatch};

#[derive(Debug)]
struct FallibleStruct {
    value: i32,
}

#[derive(Debug, Clone)]
struct FallibleStructPatch(i32);

#[derive(Debug)]
struct PatchError(String);

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PatchError: {}", self.0)
    }
}

impl std::error::Error for PatchError {}

impl Patchable for FallibleStruct {
    type Patch = FallibleStructPatch;
}

impl From<FallibleStruct> for FallibleStructPatch {
    fn from(s: FallibleStruct) -> Self {
        FallibleStructPatch(s.value)
    }
}

impl TryPatch for FallibleStruct {
    type Error = PatchError;

    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error> {
        if patch.0 < 0 {
            return Err(PatchError("Value cannot be negative".to_string()));
        }
        self.value = patch.0;
        Ok(())
    }
}

#[test]
fn test_try_patch_custom_error() {
    let mut s = FallibleStruct { value: 0 };

    // Valid patch
    assert!(s.try_patch(FallibleStructPatch(10)).is_ok());
    assert_eq!(s.value, 10);

    // Invalid patch
    let result = s.try_patch(FallibleStructPatch(-5));
    assert!(result.is_err());
    assert_eq!(s.value, 10); // Should not have changed

    match result {
        Err(e) => assert_eq!(e.to_string(), "PatchError: Value cannot be negative"),
        _ => panic!("Expected error"),
    }
}
