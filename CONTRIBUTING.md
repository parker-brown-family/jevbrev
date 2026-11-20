# Contributing

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass. `scripts/build-npm.sh` must pass for any change to the library.
- Automated tests never call Jev. A change to the questions jevbrev writes needs a calibration run by hand on real Jev answers. Record the results in the pull request, and store any answers the tests use under `tests/fixtures/`, marked measured or illustrative.
- `docs/using-brev.md` is the specification. A change in behaviour changes it in the same pull request.
- Absence is a state, not a default: an input that was not answered is `unasked`, never `unsaid`, and a value that was not read is absent, never empty.
