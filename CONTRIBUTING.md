# Contributing to RIB

Thanks for your interest in contributing! This guide keeps contributions fast, predictable, and secure.

## Quick Start
1. Fork the repo & clone your fork
2. Create a feature branch: `git checkout -b feat/my-thing`
3. Run dev infra: `docker compose up -d postgres redis minio`
4. Run backend: `cargo watch -x run`
5. (Optional) Run frontend live dev: `cd rib-react && npm install && npm run dev`
6. Add tests (see `tests/` for patterns)
7. Format & lint: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
8. Commit with a clear message
9. Open a PR against `main`

## Code Expectations
- Keep functions small & purposeful
- Public APIs must have rustdoc comments
- Avoid panics except for truly unrecoverable startup misconfig (already pattern used)
- Prefer `anyhow::Result` internally, map to API errors at handler edges
- Feature flags should default OFF unless very low risk (frontend embedding already default ON)

## Testing
- Put integration tests in `tests/`
- Use `serial_test` only when shared mutable state or external singleton is unavoidable
- Add at least one negative path test per new API handler
- Avoid sleeping in tests; use polling loops with a timeout when waiting for async side-effects

## Commit Messages
Format (loosely conventional commits):
```
feat: add thread pinning endpoint
fix: correct rate limit header casing
refactor: extract image hashing
chore: bump sqlx version
```

## Security
- Never log secrets or raw JWTs
- Do not include example secrets in code/comments
- Use constant-time comparisons for future secret validations (N/A yet)
- Report vulnerabilities privately (see `SECURITY.md`)

## Adding Dependencies
- Justify in PR description (performance, security, correctness, ergonomics)
- Prefer small, well-maintained crates
- Run `cargo deny check` locally once a deny config is added (planned)

## Style
- Follow Rust 2021 idioms
- Use `tracing` spans for complex multi-step handlers (e.g. image upload pipeline)
- Avoid over-abstraction early; prefer duplication until patterns are obvious

## Documentation
- Update `README.md` if behavior/flags/env vars change
- Add architectural decision records to the Decision Log section rather than new top-level files

## Release Process (Future)
Until versioned releases are established:
- Every merge to `main` must be green (tests & build)
- Tag meaningful milestones manually: `git tag -a v0.1.0 -m "MVP"`

## Questions
Open a GitHub Discussion or draft PR early if the direction is uncertain.

Happy hacking!
