# Patchable

A Rust library with for automatically deriving patch types and implementing efficient state updates for target types.

- A `Patchable` trait is provided:

- A derive macro that automatically generates a companion "patch" type for your target struct and implements `Patchable`
on the target type.

  This enables efficient partial updates of struct instances by applying patches, which is particularly useful for:

- State management in event-driven systems
- Incremental updates in streaming applications
- Serialization/deserialization of state changes

## Features

- **Automatic Patch Type Generation**: Derives a companion `State` struct for any struct annotated with `#[derive(Patchable)]`
- **Recursive Patching**: Use `#[patchable]` attribute to mark fields that require recursive patching
- **Smart Exclusion**: Respects `#[serde(skip)]` and `#[serde(skip_serializing)]`, and `PhantomData` to keep patches lean.
- **Serde Integration**: Generated patch types automatically implement `serde::Deserialize` and `Clone`
- **Generic Support**: Full support for generic types with automatic trait bound inference
- **Zero Runtime Overhead**: All code generation happens at compile time

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
patchable = "0.1.0"
```

## Usage

### Basic Example

```rust
use patchable::Patchable;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Patchable)]
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
    let patch: UserState = serde_json::from_str(&state_json).unwrap();
    
    let mut default = User::default();
    // Apply the patch
    default.patch(patch); 
    
    assert_eq!(default, user);
}
```

### Skipping Fields

Fields can be excluded from patching using serde attributes:

```rust
use patchable::Patchable;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Patchable)]
struct Measurement<T, F> {
    value: T,
    #[serde(skip)]
    compute_fn: F,
}
```

Fields marked with `#[serde(skip)]` or `#[serde(skip_serializing)]` are automatically excluded from the generated patch type.

### Nested Patchable Structs

The macro fully supports generic types:

```rust
use patchable::Patchable;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Patchable)]
struct Container<Closure> {
    #[serde(skip)]
    computation_logic: Closure, // Not a part of state
    metadata: String,
}

#[derive(Clone, Debug, Serialize, Patchable)]
struct Wrapper<T, Closure> {
    data: T,
    #[patchable]
    inner: Container<Closure>,
}
```

The macro automatically:

- Preserves only the generic parameters used in non-skipped fields
- Adds appropriate trait bounds (`Clone`, `Patchable`) based on field usage
- Generates correctly parameterized patch types

## How It Works

When you derive `Patchable` on a struct:

1. **Patch Type Generation**: A companion struct named `{StructName}State` is generated
   - Fields marked with `#[patchable]` use their own patch types (`T::Patch`)
   - Other fields are copied directly with their original types
   - Fields with `#[serde(skip)]`, `#[serde(skip_serializing)]` or `PhantomData` are excluded

2. **Trait Implementation**: The `Patchable` trait is implemented:

   ```rust
   pub trait Patchable {
       type Patch: Clone;
       fn patch(&mut self, patch: Self::Patch);
   }
   ```

3. **Patch Method**: The `patch` method updates the struct:
   - Regular fields are directly assigned from the patch
   - `#[patchable]` fields are recursively patched via their own `patch` method

## API Reference

### `#[derive(Patchable)]`

Derives the `Patchable` trait for a struct.

**Requirements:**

- Must be applied to a struct (not enums or unions)
- Does not support lifetime parameters (borrowed fields)
- Works with named, unnamed (tuple), and unit structs

### `#[patchable]` Attribute

Marks a field for recursive patching.

**Requirements:**

- The types of fields with `#[patchable]` must implement `Patchable`
- Currently only supports simple generic types (not complex types like `Vec<T>`)

### `Patchable` Trait

```rust
pub trait Patchable {
    type Patch: Clone;
    fn patch(&mut self, patch: Self::Patch);
}
```

- `Patch`: The associated patch type (automatically generated as `{StructName}State` if `#[derive(Patchable)]` is
  applied)

- `patch`: Method to apply a patch to the current instance

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.
