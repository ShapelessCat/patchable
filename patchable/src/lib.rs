// Re-export the procedural macro
pub use patchable_macro::Patchable;

/// A trait for structures that can be updated using a patch.
pub trait Patchable {
    /// The type of patch associated with this structure.
    type Patch: Clone;

    /// Applies the given patch to update the structure.
    fn patch(&mut self, patch: Self::Patch);
}

#[cfg(test)]
pub(crate) mod test {
    use std::fmt::Debug;

    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, Serialize, Deserialize, Patchable)]
    pub struct FakeMeasurement<T, ClosureType> {
        v: T,
        #[allow(dead_code)]
        #[serde(skip)]
        how: ClosureType,
    }

    #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
    pub struct MeasurementResult<T>(pub T);

    #[derive(Clone, Debug, Serialize, Patchable, PartialEq)]
    pub struct ScopedMeasurement<ScopeType, MeasurementType, MeasurementOutput> {
        current_control_level: ScopeType,
        #[patchable]
        inner: MeasurementType,
        current_base: MeasurementResult<MeasurementOutput>,
    }

    #[test]
    fn test_scoped_peek() -> anyhow::Result<()> {
        let fake_measurement = FakeMeasurement {
            v: 42,
            how: |x: &i32| *x,
        };
        let scoped_peek = ScopedMeasurement {
            current_control_level: 33u32,
            inner: fake_measurement.clone(),
            current_base: MeasurementResult(20i32),
        };
        let mut init_scoped_peek = scoped_peek.clone();

        let state: String = serde_json::to_string(&scoped_peek)?;
        let state_struct_value = serde_json::from_str(&state)?;

        init_scoped_peek.patch(state_struct_value);
        assert_eq!(state, serde_json::to_string(&init_scoped_peek)?);
        Ok(())
    }
}
