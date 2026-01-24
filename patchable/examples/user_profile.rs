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

use patchable::{Patchable, TryPatch};
use serde::{Deserialize, Serialize};
use std::fmt;

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
    address: Address,
    bio: String,
}

// ============================================================================
// Example 3: Fallible Patching with Validation
// ============================================================================

#[derive(Debug, PartialEq)]
struct AccountSettings {
    max_connections: u32,
    timeout_seconds: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct AccountSettingsPatch {
    max_connections: u32,
    timeout_seconds: u32,
}

#[derive(Debug)]
struct ValidationError(String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation error: {}", self.0)
    }
}

impl std::error::Error for ValidationError {}

impl TryPatch for AccountSettings {
    type Patch = AccountSettingsPatch;
    type Error = ValidationError;

    fn try_patch(&mut self, patch: Self::Patch) -> Result<(), Self::Error> {
        // Validate the patch before applying
        if patch.max_connections == 0 {
            return Err(ValidationError(
                "max_connections must be greater than 0".to_string(),
            ));
        }
        if patch.timeout_seconds > 300 {
            return Err(ValidationError(
                "timeout_seconds cannot exceed 300".to_string(),
            ));
        }

        // Apply the patch if validation passes
        self.max_connections = patch.max_connections;
        self.timeout_seconds = patch.timeout_seconds;
        Ok(())
    }
}

// ============================================================================
// Main Examples
// ============================================================================

fn main() {
    println!("=== Patchable Library Examples ===\n");

    example_1_basic_patching();
    println!();

    example_2_nested_structs();
    println!();

    example_3_fallible_patching();
}

fn example_1_basic_patching() {
    println!("--- Example 1: Basic Patching ---");

    // Create a user instance
    let mut user = User {
        id: 1,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        age: 30,
    };

    println!("Original user: {:?}", user);

    // Simulate receiving a patch as JSON (e.g., from an API)
    let patch_json = r#"{
        "id": 1,
        "name": "Alice Smith",
        "email": "alice.smith@example.com",
        "age": 31
    }"#;

    // Deserialize the JSON into a patch
    let patch: <User as Patchable>::Patch = serde_json::from_str(patch_json).unwrap();

    // Apply the patch
    user.patch(patch);

    println!("After patching: {:?}", user);
    assert_eq!(user.name, "Alice Smith");
    assert_eq!(user.email, "alice.smith@example.com");
    assert_eq!(user.age, 31);
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

fn example_3_fallible_patching() {
    println!("--- Example 3: Fallible Patching with Validation ---");

    let mut settings = AccountSettings {
        max_connections: 10,
        timeout_seconds: 30,
    };

    println!("Original settings: {:?}", settings);

    // Try a valid patch
    let valid_patch_json = r#"{
        "max_connections": 20,
        "timeout_seconds": 60
    }"#;

    let valid_patch: AccountSettingsPatch = serde_json::from_str(valid_patch_json).unwrap();

    match settings.try_patch(valid_patch) {
        Ok(()) => println!("✓ Valid patch applied successfully: {:?}", settings),
        Err(e) => println!("✗ Failed to apply patch: {}", e),
    }

    // Try an invalid patch (max_connections = 0)
    let invalid_patch_1_json = r#"{
        "max_connections": 0,
        "timeout_seconds": 30
    }"#;

    let invalid_patch_1: AccountSettingsPatch = serde_json::from_str(invalid_patch_1_json).unwrap();

    match settings.try_patch(invalid_patch_1) {
        Ok(()) => println!("✓ Patch applied: {:?}", settings),
        Err(e) => println!("✗ Expected error caught: {}", e),
    }

    // Settings should remain unchanged after failed patch
    assert_eq!(settings.max_connections, 20);

    // Try another invalid patch (timeout too large)
    let invalid_patch_2_json = r#"{
        "max_connections": 10,
        "timeout_seconds": 500
    }"#;

    let invalid_patch_2: AccountSettingsPatch = serde_json::from_str(invalid_patch_2_json).unwrap();

    match settings.try_patch(invalid_patch_2) {
        Ok(()) => println!("✓ Patch applied: {:?}", settings),
        Err(e) => println!("✗ Expected error caught: {}", e),
    }

    println!("Final settings (unchanged after errors): {:?}", settings);
    assert_eq!(settings.timeout_seconds, 60);
}
