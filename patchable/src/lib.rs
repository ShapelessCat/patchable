//! # Patchable
//!
//! A crate for handling partial updates to data structures.
//!
//! This crate provides the [`Patchable`], [`Patch`], and [`TryPatch`] traits, along with
//! derive macros for `Patchable` and `Patch`, and an attribute macro `patchable_model`
//! re-exported from `patchable_macro` for easy derivation.
//!
//! ## Motivation
//!
//! Many systems receive incremental updates where only a subset of fields change or can be
//! considered part of the state. This crate formalizes this pattern by defining a patch type for a
//! structure and providing a consistent way to apply such patches safely.

// Re-export the derive macros.
pub use patchable_macro::{Patch, Patchable, patchable_model};

/// A type that declares a companion patch type.
///
/// ## Usage
///
/// ```rust
/// use patchable::{Patch, Patchable};
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
/// // If we derive `Patchable` and `Patch` for `Accumulator`, the following `AccumulatorPatch` type
/// // and the `Patchable`/`Patch` implementations can be generated automatically.
/// //
/// // When deriving `Patchable`, a `From<Accumulator>` implementation is generated if the
/// // `impl_from` feature is enabled. For derived implementations, mark non-state fields with
/// // `#[patchable(skip)]` (and add `#[serde(skip)]` as needed when using serde).
///
/// // Derive `Clone` if needed by enabling "cloneable" feature or manually.
/// // "cloneable" is enabled by default.
/// #[derive(PartialEq, Deserialize)]
/// pub struct AccumulatorPatch<T> {
///     prev_control_signal: T,
///     accumulated: u32,
/// }
///
/// impl<T> Patchable for Accumulator<T> {
///     type Patch = AccumulatorPatch<T>;
/// }
///
/// impl<T> From<Accumulator<T>> for AccumulatorPatch<T> {
///     fn from(acc: Accumulator<T>) -> Self {
///         Self {
///             prev_control_signal: acc.prev_control_signal,
///             accumulated: acc.accumulated,
///         }
///     }
/// }
///
/// impl<T> Patch for Accumulator<T> {
///     #[inline(always)]
///     fn patch(&mut self, patch: Self::Patch) {
///         self.prev_control_signal = patch.prev_control_signal;
///         self.accumulated = patch.accumulated;
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
/// let accumulator_patch: AccumulatorPatch<i32> = serde_json::from_str(
///     r#"{
///         "prev_control_signal": 6,
///         "accumulated": 15
///     }"#
/// ).unwrap();
///
/// accumulator.patch(accumulator_patch);
///
/// assert_eq!(accumulator.prev_control_signal, 6i32);
/// assert_eq!(accumulator.accumulated, 15u32);
/// ```
/// Declares the associated patch type.
pub trait Patchable {
    /// The type of patch associated with this structure.
    type Patch;
}

/// A type that can be updated using its companion patch.
pub trait Patch: Patchable {
    /// Applies the given patch to update the structure.
    fn patch(&mut self, patch: Self::Patch);
}

/// A fallible variant of [`Patch`].
///
/// This trait lets you apply a patch with validation and return a custom error
/// if it cannot be applied.
///
/// ## Usage
///
/// ```rust
/// use patchable::{TryPatch, Patchable};
/// use std::fmt;
///
/// #[derive(Debug)]
/// struct Config {
///     concurrency: u32,
/// }
///
/// #[derive(Clone, PartialEq)]
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
/// impl Patchable for Config {
///     type Patch = ConfigPatch;
/// }
///
/// impl From<Config> for ConfigPatch {
///     fn from(c: Config) -> Self {
///         Self { concurrency: c.concurrency }
///     }
/// }
///
/// impl TryPatch for Config {
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
pub trait TryPatch: Patchable {
    /// The error type returned when applying a patch fails.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Applies the provided patch to `self`.
    ///
    /// # Errors
    ///
    /// Returns an error if the patch is invalid or cannot be applied.
    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error>;
}

/// Blanket implementation for all [`Patch`] types, where patching is
/// infallible.
impl<T: Patch> TryPatch for T {
    type Error = std::convert::Infallible;

    #[inline(always)]
    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error> {
        self.patch(patch);
        Ok(())
    }
}

/// Implementation for `Box<T>`
impl<T: Patchable> Patchable for Box<T> {
    type Patch = Box<T::Patch>;
}

impl<T: Patch> Patch for Box<T> {
    fn patch(&mut self, patch: Self::Patch) {
        self.as_mut().patch(*patch);
    }
}

/// Implementation for `Option<T>`
impl<T: Patchable> Patchable for Option<T> {
    type Patch = Option<T::Patch>;
}

impl<T: Patch> Patch for Option<T> {
    fn patch(&mut self, patch: Self::Patch) {
        if let (Some(s), Some(p)) = (self, patch) {
            s.patch(p);
        }
    }
}

/// Implementation for `Vec<T>` (Full Replacement)
impl<T> Patchable for Vec<T> {
    type Patch = Vec<T>;
}

impl<T> Patch for Vec<T> {
    fn patch(&mut self, patch: Self::Patch) {
        *self = patch;
    }
}

#[cfg(test)]
pub(crate) mod test {
    use std::fmt::Debug;

    use super::*;
    use patchable_macro::patchable_model;
    use serde::{Deserialize, Serialize};

    #[patchable_model]
    #[derive(Clone, Default, Debug, PartialEq, Deserialize)]
    #[serde(bound(
        serialize = "T: ::serde::Serialize",
        deserialize = "T: ::serde::Deserialize<'de>"
    ))]
    struct FakeMeasurement<T, ClosureType> {
        v: T,
        #[patchable(skip)]
        how: Option<ClosureType>,
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
        fn identity(x: &i32) -> i32 {
            *x
        }

        let fake_measurement: FakeMeasurement<i32, fn(&i32) -> i32> = FakeMeasurement {
            v: 42,
            how: Some(identity),
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
        let patch: <SimpleStruct as Patchable>::Patch =
            serde_json::from_str(r#"{"val": 20}"#).unwrap();

        // Should always succeed for `Patch` types due to the blanket impl.
        let result = s.try_patch(patch);
        assert!(result.is_ok());
        assert_eq!(s.val, 20);
    }

    #[allow(dead_code)]
    #[patchable_model]
    #[derive(Clone, Debug, PartialEq, Deserialize)]
    struct Inner {
        value: i32,
    }

    #[allow(dead_code)]
    #[patchable_model]
    #[derive(Clone, Debug, PartialEq)]
    struct Outer<InnerType> {
        #[patchable]
        inner: InnerType,
        extra: u32,
    }

    // TODO: Not testing `impl_from` feature. Need fix.
    #[cfg(feature = "impl_from")]
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

    #[derive(Debug)]
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

    impl Patchable for FallibleStruct {
        type Patch = FalliblePatch;
    }

    impl From<FallibleStruct> for FalliblePatch {
        fn from(s: FallibleStruct) -> Self {
            FalliblePatch(s.value)
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
