# Patchable

[![CI](https://github.com/ShapelessCat/patchable/actions/workflows/ci.yaml/badge.svg)](https://github.com/ShapelessCat/patchable/actions/workflows/ci.yaml)
[![Crates.io](https://img.shields.io/crates/v/patchable.svg)](https://crates.io/crates/patchable)
[![Documentation](https://docs.rs/patchable/badge.svg)](https://docs.rs/patchable)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A Rust library for automatically deriving patch types and implementing efficient updates from patches for target types.

This project provides:

- A `Patchable` trait for declaring patch types.
- A `Patch` trait for applying partial updates.
- A `TryPatch` trait as a fallible version of `Patch`.
- Derive macros that generate companion patch types (`Patchable`) and infallible patch logic (`Patch`).

This enables efficient partial updates of struct instances by applying patches, which is particularly useful for:

- State management in event-driven systems.
- Incremental updates in streaming applications.
- Serialization/deserialization of state changes.

Note: patch types intentionally do not derive `Serialize`; patches should be created from their companion structs. The
"serialization" item above refers to serializing a `Patchable` type to produce its companion patch type instance.

## Why Patchable?

Patchable shines when you need to persist and update state without hand-maintaining
parallel "state" structs. A common example is durable execution: save only true
state while skipping non-state fields (caches, handles, closures), then restore
or update state incrementally.

The provided derive macros handle the heavy lifting:

1. **Patch Type Definition**: For a given a struct definition, `#[derive(Patchable)]` provides
   fine-grained control over what becomes part of its companion patch:

   - Exclude **non-state fields**.
   - Include **simple fields** directly.
   - Include **complex fields**, which have their own patch types, indirectly by including their patches.

   When `#[derive(Patchable)]` is used, a `From<Struct>` for `StructPatch` can be generated
   by enabling the `impl_from` feature.

2. **Correct Patch Behavior**: The macro generates `Patch` implementations and
   correct `patch` methods based on the rules in item 1.

3. **Deserializable Patches**: Patches can be decoded for storage or transport.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [Basic Example](#basic-example)
  - [Skipping Fields](#skipping-fields)
  - [Nested Patchable Structs](#nested-patchable-structs)
  - [Fallible Patching](#fallible-patching)
- [How It Works](#how-it-works)
- [API Reference](#api-reference)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Automatic Patch Type Generation**: Derives a companion `Patch` struct for any struct annotated with `#[derive(Patchable)]`
- **Recursive Patching**: Use `#[patchable]` attribute to mark fields that require recursive patching
- **Smart Exclusion**: Respects `#[serde(skip)]` and `#[serde(skip_serializing)]`, and `PhantomData` to keep patches lean.
- **Serde Integration**: Generated patch types automatically implement `serde::Deserialize` and `Clone`
- **Generic Support**: Full support for generic types with automatic trait bound inference
- **Optional `From` Derive**: Enable `From<Struct>` for `StructPatch` with the `impl_from` feature
- **Zero Runtime Overhead**: All code generation happens at compile time

## Use Cases

Patchable is a good fit when you want to update state without hand-maintaining parallel structs, such as:

- Event-sourced or durable systems where only state fields should be persisted.
- Streaming or real-time pipelines that receive incremental updates.
- Syncing or transporting partial state over the network.

## Installation

**MSRV:** Rust 1.85 (edition 2024).

Add this to your `Cargo.toml`:

```toml
[dependencies]
patchable = "0.5.0" # You can use the latest version
```

Enable `From<Struct>` generation:

```toml
[dependencies]
patchable = { version = "0.5.0", features = ["impl_from"] }
```

## Usage

### Basic Example

```rust
use patchable::{Patch, Patchable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Patchable, Patch)]
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

### Skipping Fields

Fields can be excluded from patching using serde attributes:

```rust
use patchable::{Patch, Patchable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Patchable, Patch)]
struct Measurement<T, F> {
    value: T,
    #[serde(skip)]
    compute_fn: F,
}
```

Fields marked with `#[serde(skip)]` or `#[serde(skip_serializing)]` are automatically excluded from the generated patch type.

### Nested Patchable Structs

The macros fully support generic types:

```rust
use patchable::{Patch, Patchable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Patchable, Patch)]
struct Container<Closure> {
    #[serde(skip)]
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

- Preserves only the generic parameters used in non-skipped fields
- Adds appropriate trait bounds (`Clone`, `Patchable`, `Patch`) based on field usage
- Generates correctly parameterized patch types

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
- Generated patch types derive `Clone` and `Deserialize` but not `Serialize` (by design).

## How It Works

When you derive `Patchable` on a struct:

1. **Patch Type Generation**: A companion struct named `{StructName}Patch` is generated
   - Fields marked with `#[patchable]` use their own patch types (`T::Patch`)
   - Other fields are copied directly with their original types
   - Fields with `#[serde(skip)]`, `#[serde(skip_serializing)]` or `PhantomData` are excluded

2. **Trait Implementation**: The `Patchable` trait is implemented:

   ```rust
   pub trait Patchable {
       type Patch: Clone;
   }
   ```

When you derive `Patch` on a struct:

1. **Patch Method**: The `patch` method updates the struct:
   - Regular fields are directly assigned from the patch
   - `#[patchable]` fields are recursively patched via their own `patch` method

2. **Trait Implementation**: The `Patch` trait is implemented:

   ```rust
   pub trait Patch: Patchable {
       fn patch(&mut self, patch: Self::Patch);
   }
   ```

## API Reference

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
    type Patch: Clone;
}
```

- `Patch`: The associated patch type (automatically generated as `{StructName}Patch` if `#[derive(Patchable)]` is
  applied)

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
