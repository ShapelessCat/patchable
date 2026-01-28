# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - 2026-01-28

### Added

- Added Cargo feature `impl_from` to automatically implement `From<Struct>` for its patch type
  `StructPatch` when using `#[derive(Patchable)]`.

### Changed

- `Patchable` now requires `Sized`, so it is no longer Dyn-compatible

## [0.5.0] - 2026-01-27

### Changed

- Renamed core traits: `WithPatch` -> `Patchable`, `Patchable` -> `Patch`
- Renamed derive macros to match the new trait names (`Patchable`, `Patch`)
- Updated README, examples, and macro docs to reflect the new naming and behavior

## [0.4.1] - 2026-01-26

### Added

- Manifest categories and keywords for better discoverability
- Tests for tuple struct and unit struct patching
- CHANGELOG.md, CONTRIBUTING.md, and VSCode file associations

### Changed

- Improved error handling in `MacroContext::new` for struct validation
- Updated project licenses and manifests
- Refined README and doc comments (examples, descriptions, badge icon)

## [0.4.0] - 2026-01-24

### Added

- `TryPatch` trait for fallible patch operations
- Support for validation during patch application
- Comprehensive API documentation

### Changed

- Enhanced error handling in derive macro
- Improved documentation with more examples

## [0.1.0], [0.2.x], and [0.3.0]

Early development, and you shouldn't use these versions for your projects.

### Added

- Initial `Patchable` trait and derive macro
- Automatic patch type generation
- Basic field patching functionality

[0.5.1]: https://github.com/ShapelessCat/patchable/releases/tag/v0.5.1
[0.5.0]: https://github.com/ShapelessCat/patchable/releases/tag/v0.5.0
[0.4.1]: https://github.com/ShapelessCat/patchable/releases/tag/v0.4.1
[0.4.0]: https://github.com/ShapelessCat/patchable/releases/tag/v0.4.0
