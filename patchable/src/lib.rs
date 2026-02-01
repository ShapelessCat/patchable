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
