# Contributing to Patchable

Thank you for your interest in contributing to Patchable! We welcome contributions from the community.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/patchable.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Prerequisites

- Rust (latest stable version)
- Cargo

### Building the Project

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

<!-- ### Running Examples

TODO: Will add soon

```bash
cargo run --example example_name
``` -->

## Making Changes

### Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Run `cargo fmt` before committing
- Run `cargo clippy` and address any warnings

### Documentation

- Add doc comments for all public APIs
- Include examples in doc comments where applicable
- Update README.md if adding new features

### Testing

- Write tests for new functionality
- Ensure all existing tests pass
- Add integration tests for significant features

## Submitting Changes

1. Commit your changes with clear, descriptive commit messages
2. Push to your fork: `git push origin feature/your-feature-name`
3. Open a Pull Request against the main repository
4. Describe your changes in the PR description
5. Link any related issues

## Pull Request Guidelines

- Keep PRs focused on a single feature or bug fix
- Include tests for new functionality
- Update documentation as needed
- Ensure CI passes before requesting review

## Reporting Bugs

When reporting bugs, please include:

- A clear description of the issue
- Steps to reproduce
- Expected behavior
- Actual behavior
- Rust version and OS
- Minimal code example demonstrating the issue

## Feature Requests

We welcome feature requests! Please:

- Check if the feature has already been requested
- Clearly describe the use case
- Explain why it would be valuable to the project
- Consider submitting a PR if you're able to implement it

## Code of Conduct

Please be respectful and constructive in all interactions. We aim to foster an inclusive and welcoming community.

## Questions?

Feel free to open an issue for questions or discussions about the project.

## License

By contributing to *patchable*, you agree that your contributions will be licensed under the MIT License and Apache-2.0 License.
