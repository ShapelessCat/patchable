//! A comprehensive example demonstrating the patchable library.
//!
//! This example shows:
//! - Basic usage of the `Patchable` derive macro
//! - Serialization and deserialization of patches
//! - Nested patchable structs
//! - Skipping fields with `#[serde(skip)]`
//! - Fallible patching with `TryPatch`
//!
//! Run with: `cargo run --example user_profile`

use patchable::Patchable;
use serde::{Deserialize, Serialize};

// ============================================================================
// Example 1: Basic Patchable Struct
// ============================================================================

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq, Patchable)]
struct User {
    id: u64,
    name: String,
    email: String,
    age: u32,
}

// ============================================================================
// Example 2: Nested Patchable Structs with Skipped Fields
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Patchable)]
struct Address {
    street: String,
    city: String,
    postal_code: String,
    // This field is skipped - it won't appear in the patch type
    #[serde(skip, default = "default_validation_fn")]
    validation_fn: fn(&str) -> bool,
}

fn validation_fn(_bio: &str) -> bool {
    true
}

fn default_validation_fn() -> fn(&str) -> bool {
    validation_fn
}

#[derive(Clone, Debug, Serialize, Patchable)]
struct UserProfile {
    username: String,
    #[patchable]
    address: Address,
    bio: String,
}

// ============================================================================
// Main Examples
// ============================================================================

fn main() {
    println!("=== Patchable Library Examples ===\n");

    example_2_nested_structs();
    println!();
}

fn example_2_nested_structs() {
    println!("--- Example 2: Nested Patchable Structs ---");

    // Validation function that will be skipped
    fn validate_bio(bio: &str) -> bool {
        bio.len() <= 500
    }

    let mut profile = UserProfile {
        username: "alice42".to_string(),
        address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            postal_code: "12345".to_string(),
            validation_fn: validate_bio,
        },
        bio: "Software developer".to_string(),
    };

    println!("Original profile: {:#?}", profile);

    // Create a patch that updates both the profile and nested address
    let patch_json = r#"{
        "username": "alice42",
        "address": {
            "street": "456 Oak Ave",
            "city": "Portland",
            "postal_code": "97201"
        },
        "bio": "Senior software engineer and open source contributor"
    }"#;

    // Note: The validation_fn field is not in the JSON because it's skipped
    let patch: <UserProfile as Patchable>::Patch = serde_json::from_str(patch_json).unwrap();

    // Apply the patch - notice how nested structs are also patched
    profile.patch(patch);

    println!("After patching: {:#?}", profile);
    assert_eq!(profile.address.city, "Portland");
    assert_eq!(
        profile.bio,
        "Senior software engineer and open source contributor"
    );
}
