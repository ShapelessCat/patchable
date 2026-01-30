# Patchable

[![CI](https://github.com/ShapelessCat/patchable/actions/workflows/ci.yaml/badge.svg)](https://github.com/ShapelessCat/patchable/actions/workflows/ci.yaml)
[![Crates.io](https://img.shields.io/crates/v/patchable.svg)](https://crates.io/crates/patchable)
[![Documentation](https://docs.rs/patchable/badge.svg)](https://docs.rs/patchable)
[![patchable MSRV](https://img.shields.io/crates/msrv/patchable.svg?label=patchable%20msrv&color=lightgray)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)
[![patchable-macro MSRV](https://img.shields.io/crates/msrv/patchable-macro.svg?label=patchable-macro%20msrv&color=lightgray)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A Rust library for deriving patch types and applying patches efficiently to update target types.

Patchable gives each struct a companion patch type plus trait implementations for applying partial
updates. It focuses on compact patch representations and efficient updates.

Note:
Each struct has one companion patch struct, and each patch struct corresponds to one struct.

## Why Patchable?

Patchable shines when you need to persist and update state without hand-maintaining
parallel state structs. A common use case is durable execution: save only true
state while skipping non-state fields (caches, handles, closures), then restore
or update state incrementally.

Typical scenarios include:

- Durable or event-sourced systems where only state fields should be persisted.
- Streaming or real-time pipelines that receive incremental updates.
- Syncing or transporting partial state over the network.

The provided derive macros handle the heavy lifting; they generate companion patch types and patch
logic. See Features and How It Works for details.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [Basic Example](#basic-example)
  - [Using `#[patchable_model]`](#using-patchable_model)
  - [Skipping Fields](#skipping-fields)
  - [Nested Patchable Structs](#nested-patchable-structs)
  - [Fallible Patching](#fallible-patching)
- [How It Works](#how-it-works)
- [API Reference](#api-reference)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Automatic Patch Type Generation**: Derives a companion `Patch` struct for any struct annotated with `#[derive(Patchable)]`
- **Recursive Patching**: Use the `#[patchable]` attribute to mark fields that require recursive patching
- **Smart Exclusion**: Excludes fields marked with `#[patchable(skip)]`
- **Serde Integration (optional, default)**: Generated patch types automatically implement `serde::Deserialize` (exclude
  the `serde` feature to opt out)
- **Clone Support (optional, default)**: Generated patch types automatically implement `Clone` (exclude the `cloneable`
  feature to opt out)
- **Generic Support**: Full support for generic types with automatic trait bound inference
- **Optional `From` Derive**: Enable `From<Struct>` for `StructPatch` with the `impl_from` feature
- **`#[patchable_model]` Attribute Macro**: Auto-derives `Patchable` and `Patch`, and (with default `serde`) adds `serde::Serialize`
- **Zero Runtime Overhead**: All code generation happens at compile time

## Installation

**MSRV:** Rust 1.85 (edition 2024).

Add this to your `Cargo.toml`:

```toml
[dependencies]
patchable = "0.5.3" # You can use the latest version
```

The `serde` feature is enabled by default. Disable default features to opt out:

```toml
[dependencies]
patchable = { version = "0.5.3", default-features = false }
```

Enable `From<Struct>` generation:

```toml
[dependencies]
patchable = { version = "0.5.3", features = ["impl_from"] }
```

Enable `Clone` derivation for patch types:

```toml
[dependencies]
patchable = { version = "0.5.3", features = ["cloneable"] }
```

## Usage

### Basic Example

```rust
use patchable::{Patch, Patchable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Patchable, Patch)]
struct User {
    id: u64,
    name: String,
    email: String,
}

fn main() {
    let mut user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };

    // Serialize the current state
    let state_json = serde_json::to_string(&user).unwrap();

    // Deserialize into a patch
    let patch: UserPatch = serde_json::from_str(&state_json).unwrap();

    let mut default = User::default();
    // Apply the patch
    default.patch(patch);

    assert_eq!(default, user);
}
```

### Using `#[patchable_model]`

The simplest way to use this library is the attribute macro:

```rust
use patchable::patchable_model;

#[patchable_model]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct User {
    id: u64,
    name: String,
    #[patchable(skip)]
    cache_key: String,
}
```

`#[patchable_model]` always adds `Patchable` and `Patch` derives. With the default
`serde` feature enabled, it also adds `serde::Serialize` and injects `#[serde(skip)]`
for fields marked `#[patchable(skip)]`.
Add any other derives you need (for example, `Deserialize`) alongside it.

### Skipping Fields

Fields can be excluded from patching using `#[patchable(skip)]`:

```rust
use patchable::patchable_model;
use serde::Deserialize;

#[patchable_model]
#[derive(Clone, Debug, Deserialize)]
struct Measurement<T, F> {
    value: T,
    #[patchable(skip)]
    compute_fn: F,
}
```

Fields marked with `#[patchable(skip)]` are excluded from the generated patch type. If you use
`#[patchable_model]` with the default `serde` feature enabled, those fields also receive
`#[serde(skip)]` so serialized state and patches stay aligned.
If you derive `Patchable`/`Patch` directly, add `#[serde(skip)]` yourself when you want
serialization to match patching behavior.

### Nested Patchable Structs

The macros fully support generic types:

```rust
use patchable::{Patch, Patchable};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Patchable, Patch)]
struct Container<Closure> {
    #[serde(skip)]
    #[patchable(skip)]
    computation_logic: Closure, // Not a part of state
    metadata: String,
}

#[derive(Clone, Debug, Serialize, Patchable, Patch)]
struct Wrapper<T, Closure> {
    data: T,
    #[patchable]
    inner: Container<Closure>,
}
```

The macros automatically:

- Preserve only the generic parameters used by non-skipped fields
- Add appropriate trait bounds (`Clone`, `Patchable`, `Patch`) based on field usage
- Generate correctly parameterized patch types

### Fallible Patching

The `TryPatch` trait allows for fallible updates, which is useful when patch application requires validation:

```rust
use patchable::{TryPatch, Patchable};
use std::fmt;

struct Config {
    limit: u32,
}

#[derive(Clone)]
struct ConfigPatch {
    limit: u32,
}

#[derive(Debug)]
struct InvalidConfigError;

impl fmt::Display for InvalidConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "limit cannot be zero")
    }
}

impl std::error::Error for InvalidConfigError {}

impl Patchable for Config {
    type Patch = ConfigPatch;
}

impl TryPatch for Config {
    type Error = InvalidConfigError;

    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error> {
        if patch.limit == 0 {
            return Err(InvalidConfigError);
        }
        self.limit = patch.limit;
        Ok(())
    }
}
```

### Limitations

- Only structs are supported (enums and unions are not).
- Lifetime parameters are not supported.
- `#[patchable]` currently only supports simple generic types (not complex types like `Vec<T>`).
- Generated patch types derive `Deserialize` (default) and `Clone` (optional with `cloneable` feature) but not `Serialize` (by design).

## How It Works

When you derive `Patchable` on a struct, for instance, `Struct`:

1. **Companion Patch Type**: The macro generates `StructPatch`, which mirrors the original
   structure but only includes fields that are part of the patch. Here are the rules:
   - Each field marked with `#[patchable]` in `Struct` are typed with
     `<FieldType as Patchable>::Patch` in `StructPatch`.
   - Fields marked with `#[patchable(skip)]` are excluded.
   - The left fields are copied directly with their original types.

2. **Trait Implementation**: The macro implements `Patchable` for `Struct` and sets
   `type Patch = StructPatch` (see the API reference for the exact trait definition).

3. **Serialized State to Patch**: If you serialize a `Struct` instance, that serialized value can
   be deserialized into `<Struct as Patchable>::Patch`, which yields a patch representing the
   serialized state.

When you derive `Patch` on a struct:

1. **Patch Method**: The `patch` method updates the struct:
   - Regular fields are directly assigned from the patch
   - `#[patchable]` fields are recursively patched via their own `patch` method

2. **Trait Implementation**: The macro generates `Patch` implementation for the target struct (see
API reference for the exact trait definitions).

## API Reference

### `#[patchable_model]`

Attribute macro that injects `Patchable` and `Patch` derives for a struct.

**Behavior:**

- Adds `#[derive(Patchable, Patch)]` to the target struct.
- With the default `serde` feature enabled, it also derives `serde::Serialize` and
  applies `#[serde(skip)]` to fields annotated with `#[patchable(skip)]`.

### `#[derive(Patchable)]`

Generates the companion `{StructName}Patch` type and implements `Patchable` for a struct.

**Requirements:**

- Must be applied to a struct (not enums or unions)
- Does not support lifetime parameters (borrowed fields)
- Works with named, unnamed (tuple), and unit structs

### `#[derive(Patch)]`

Derives the `Patch` trait implementation for a struct.

**Requirements:**

- Must be applied to a struct (not enums or unions)
- Does not support lifetime parameters (borrowed fields)
- Works with named, unnamed (tuple), and unit structs
- The target type must implement `Patchable` (derive it or implement manually)

### `#[patchable]` Attribute

Marks a field for recursive patching.

**Requirements:**

- The types of fields with `#[patchable]` must implement `Patch`
- Currently only supports simple generic types (not complex types like `Vec<T>`)

### `Patchable` Trait

```rust
pub trait Patchable {
    type Patch;
}
```

- `Patch`: The associated patch type (automatically generated as `{StructName}Patch` when `#[derive(Patchable)]`
  is applied)

### `Patch` Trait

```rust
pub trait Patch: Patchable {
    fn patch(&mut self, patch: Self::Patch);
}
```

- `patch`: Method to apply a patch to the current instance

### `TryPatch` Trait

A fallible variant of `Patch` for cases where applying a patch might fail.

```rust
pub trait TryPatch: Patchable {
    type Error: std::error::Error + Send + Sync + 'static;
    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error>;
}
```

- `try_patch`: Applies the patch, returning a `Result`. A blanket implementation exists for all types that implement
  `Patch` (where `Error` is `std::convert::Infallible`).

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started.

## License

This project is licensed under the [MIT License](LICENSE-MIT) and [Apache-2.0 License](LICENSE-APACHE).

## Related Projects

- [serde](https://serde.rs/) - Serialization framework that integrates seamlessly with Patchable

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.
