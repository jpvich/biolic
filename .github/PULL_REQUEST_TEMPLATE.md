## What this changes

A short description of the change and the motivation. Link any related issue
(e.g. `Closes #12`).

## Type

- [ ] New module
- [ ] Improvement to an existing module
- [ ] Core / I/O / output change
- [ ] Docs / tooling

## Checklist

- [ ] Targets the `dev` branch
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test` passes (incl. at least one integration test for new behavior)
- [ ] Streaming preserved (O(1) memory) or any O(N) cost documented
- [ ] README / CHANGELOG updated if user-facing
- [ ] For a new module: follows the Module Contract (`ARCHITECTURE.md`) and
      was discussed in a new-module proposal issue

## Notes for reviewers

Anything specific you want feedback on.
