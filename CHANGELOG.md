# Changelog

## 0.1.0 — 2026-11-20

First release.

- The `.brev` format: actions, inputs with words, role groups, and the types `str`, `int`, `date`, `weekday`, `time`, `one:A|B` and `ref:ACTION`.
- `check`, with every problem and its line.
- The two Jev requests (`menu`, `frames`), and readers for their answers (`read_action`, `read`), with four input states: `said`, `unsaid`, `unsure`, `unasked`.
- `http_call` and `ask_for`.
- `import` from OpenAPI 3 JSON, and `llm`, `tools` and `openapi` outputs.
- The Rust crate, the `jevbrev` command, and an npm package built to WebAssembly from the same code.
