# Changelog

## Unreleased

- The README now says what jevbrev is: a condensed API definition, and the outputs generated from it. It explains why the format exists, how it changes the work, and what it costs, with measured numbers.
- New example, `examples/shop.brev`, used by the README and the getting-started guide. A test checks every example loads and that the README quotes the shop example as it is.
- The appointment request used through the specification and tests is replaced by a new one, `move Alex's Tuesday 10:00s to Thursday at 14:30 from next month`. Its stored Jev answers (`tests/fixtures/answers.move.json`) were re-measured on real Jev, so every answer in it is now measured.
- The documentation says "the person" where it said "she" or "her".
- Package descriptions now say what the package is.

## 0.2.0 — 2026-11-20

- `jevbrev import OPENAPI.json --into FILE` updates an existing `.brev` after the API changes. It keeps every word, summary, confirmed read or change, type, role and example a person wrote. It adds new actions and inputs as unwritten, lists removed ones in comments at the end of the file, and reports what changed. Also `Brev.mergeOpenapi` in the npm package and `import_into` in the crate.
- `jevbrev fmt FILE` prints a file in the standard layout.
- `docs/getting-started.md` for people, `docs/agents.md` for coding agents, and an agent skill in `skills/jevbrev/`.
- The README now starts with what jevbrev does in plain words, then the technical detail, then a section for agents covering what to read, security and disclaimers.

## 0.1.1 — 2026-11-20

- The npm package is `@parker-brown-family/jevbrev`. The READMEs published with 0.1.0 named it `jevbrev`, which is not on npm. No code changed.

## 0.1.0 — 2026-11-20

First release.

- The `.brev` format: actions, inputs with words, role groups, and the types `str`, `int`, `date`, `weekday`, `time`, `one:A|B` and `ref:ACTION`.
- `check`, with every problem and its line.
- The two Jev requests (`menu`, `frames`), and readers for their answers (`read_action`, `read`), with four input states: `said`, `unsaid`, `unsure`, `unasked`.
- `http_call` and `ask_for`.
- `import` from OpenAPI 3 JSON, and `llm`, `tools` and `openapi` outputs.
- The Rust crate, the `jevbrev` command, and an npm package built to WebAssembly from the same code.
