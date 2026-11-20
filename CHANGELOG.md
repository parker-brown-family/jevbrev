# Changelog

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
