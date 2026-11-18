## Description

<!-- Provide a brief description of the changes introduced by this pull request. -->

## Related Issues

<!-- Link to any related issues (e.g. Closes #123, Fixes #456). -->

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] UI / Styling enhancement (themes, layouts, visual components)
- [ ] Documentation update (markdown docs, manual pages, doc comments)
- [ ] Refactoring or code cleanliness (no functional change)
- [ ] Performance improvement
- [ ] Translation / Internationalization (i18n)

## Contributor Pre-Flight Checklist

Please ensure your changes conform to the project's engineering invariants before submitting:

- [ ] My code follows the code style and formatting standards (`make fmt` or `cargo fmt --all --check`).
- [ ] Clippy checks pass without warnings (`make clippy` or `cargo clippy --workspace --all-targets -- -D warnings`).
- [ ] All unit and integration tests pass (`make test` or `cargo test --workspace`).
- [ ] Shell scripts pass shellcheck (`make shellcheck` or `shellcheck *.sh scripts/*.sh`).
- [ ] I have verified terminal behavior manually in a terminal emulator.
- [ ] Interactive UI code does not introduce unhandled `.unwrap()` or `.expect()` calls in event loops.
- [ ] Visual layouts maintain the 4-column icon slot and 50/50 marquee invariants.
- [ ] If adding or changing macros, placeholders, or directives, I have updated all 6 documentation tiers (see [CONTRIBUTING.md](CONTRIBUTING.md)).
