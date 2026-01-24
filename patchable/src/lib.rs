//! # Patchable
//!
//! A crate for handling partial updates to data structures.
//!
//! This crate provides the [`Patchable`] and [`TryPatch`] traits, along with a
//! derive macro [`patchable_macro::Patchable`] for easy implementation.
//!
//! ## Motivation
//!
//! Many systems receive incremental updates where only a subset of fields change or can be
//! considered as parts of state. This crate formalizes this pattern by defining a patch type for a
//! structure and providing a consistent way to apply such patches safely.

// Re-export the procedural macro
pub use patchable_macro::Patchable;

/// A data structure that can be updated using a corresponding patch.
///
/// ## Usage
///
/// ```rust
/// use patchable::Patchable;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize)]
/// pub struct Accumulator<T> {
///     prev_control_signal: T,
///     #[serde(skip)]
///     filter: fn(&i32) -> bool,
///     accumulated: u32,
/// }
///
/// //vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
/// // If we derive `Patchable` for `Accumulator`, the following `AccumulatorState` and `Patchable`
/// // implementation for `Accumulator` can be generated automatically.
///
/// #[derive(Clone, Deserialize)]
/// pub struct AccumulatorState<T> {
///     prev_control_signal: T,
///     accumulated: u32,
/// }
///
/// impl<T> Patchable for Accumulator<T>
/// where
///     T: Clone,
/// {
///     type Patch = AccumulatorState<T>;
///
///     #[inline(always)]
///     fn patch(&mut self, state: Self::Patch) {
///         self.prev_control_signal = state.prev_control_signal;
///         self.accumulated = state.accumulated;
///     }
/// }
/// //^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///
/// let mut accumulator = Accumulator {
///     prev_control_signal: -1,
///     filter: |x: &i32| *x > 300,
///     accumulated: 0,
/// };
///
/// let accumulator_state: AccumulatorState<i32> = serde_json::from_str(
///     r#"{
///         "prev_control_signal": 6,
///         "accumulated": 15
///     }"#
/// ).unwrap();
///
/// accumulator.patch(accumulator_state);
///
/// assert_eq!(accumulator.prev_control_signal, 6i32);
/// assert_eq!(accumulator.accumulated, 15u32);
/// ```
pub trait Patchable {
    /// The type of patch associated with this structure.
    type Patch: Clone;

    /// Applies the given patch to update the structure.
    fn patch(&mut self, patch: Self::Patch);
}

/// A fallible variant of [`Patchable`].
///
/// This trait allows applying a patch with validation, returning a custom error
/// if the patch cannot be applied.
///
/// ## Usage
///
/// ```rust
/// use patchable::TryPatch;
/// use std::fmt;
///
/// #[derive(Debug, PartialEq)]
/// struct Config {
///     concurrency: u32,
/// }
///
/// #[derive(Clone)]
/// struct ConfigPatch {
///     concurrency: u32,
/// }
///
/// #[derive(Debug)]
/// struct PatchError(String);
///
/// impl fmt::Display for PatchError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "{}", self.0)
///     }
/// }
///
/// impl std::error::Error for PatchError {}
///
/// impl TryPatch for Config {
///     type Patch = ConfigPatch;
///     type Error = PatchError;
///
///     fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error> {
///         if patch.concurrency == 0 {
///             return Err(PatchError("Concurrency must be > 0".into()));
///         }
///         self.concurrency = patch.concurrency;
///         Ok(())
///     }
/// }
///
/// let mut config = Config { concurrency: 1 };
/// let valid_patch = ConfigPatch { concurrency: 4 };
/// config.try_patch(valid_patch).unwrap();
/// assert_eq!(config.concurrency, 4);
///
/// let invalid_patch = ConfigPatch { concurrency: 0 };
/// assert!(config.try_patch(invalid_patch).is_err());
/// ```
pub trait TryPatch {
    /// The type of patch associated with this structure.
    type Patch: Clone;

    /// The error type returned when applying a patch fails.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Applies the provided patch to `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch is invalid or cannot be applied.
    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error>;
}

/// Blanket implementation for all [`Patchable`] types, where patching is
/// infallible.
impl<T: Patchable> TryPatch for T {
    type Patch = T::Patch;
    type Error = std::convert::Infallible;

    #[inline(always)]
    fn try_patch(&mut self, state: Self::Patch) -> Result<(), Self::Error> {
        self.patch(state);
        Ok(())
    }
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

    #[derive(Clone, Default, Debug, Serialize, Deserialize, Patchable)]
    struct SimpleStruct {
        val: i32,
    }

    #[test]
    fn test_try_patch_blanket_impl() {
        let mut s = SimpleStruct { val: 10 };
        // The derived patch struct is compatible with serde.
        // We use from_str to create the patch.
        let patch: <SimpleStruct as Patchable>::Patch =
            serde_json::from_str(r#"{"val": 20}"#).unwrap();

        // Should always succeed for Patchable types due to blanket impl
        let result = s.try_patch(patch);
        assert!(result.is_ok());
        assert_eq!(s.val, 20);
    }

    #[derive(Debug, PartialEq)]
    struct FallibleStruct {
        value: i32,
    }

    #[derive(Debug, Clone)]
    struct FalliblePatch(i32);

    #[derive(Debug)]
    struct PatchError(String);

    impl std::fmt::Display for PatchError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "PatchError: {}", self.0)
        }
    }

    impl std::error::Error for PatchError {}

    impl TryPatch for FallibleStruct {
        type Patch = FalliblePatch;
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
        assert!(s.try_patch(FalliblePatch(10)).is_ok());
        assert_eq!(s.value, 10);

        // Invalid patch
        let result = s.try_patch(FalliblePatch(-5));
        assert!(result.is_err());
        assert_eq!(s.value, 10); // Should not have changed

        match result {
            Err(e) => assert_eq!(e.to_string(), "PatchError: Value cannot be negative"),
            _ => panic!("Expected error"),
        }
    }
}
