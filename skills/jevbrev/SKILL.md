---
name: jevbrev
description: Describe an HTTP API in a .brev file so code can ask Jev (TypeSafe System One) which action a person's request wants and what inputs it gives, then read the answers and build the HTTP call. Use when putting an API behind a natural-language assistant with Jev, when asked to "write a .brev", "import our OpenAPI into jevbrev", "calibrate the .brev", "wire jevbrev into the app", "the API changed, update the .brev", or when a .brev file or the jevbrev command or package appears in the work.
license: MIT OR Apache-2.0
---

# jevbrev

A `.brev` file is a condensed API definition: one line per action and one per input, in the words a person would use. jevbrev turns it into Jev's questions, reads Jev's answers, builds the HTTP call, and prints compact definitions for language models. It never calls Jev and never holds a key.

The full guide is `docs/agents.md` in https://github.com/parker-brown-family/jevbrev. The specification is `docs/using-brev.md`, with every request and answer shown. Read both before your first `.brev`. This page is the short version.

## Install

`cargo install jevbrev` (command), `cargo add jevbrev` (Rust), `npm install @parker-brown-family/jevbrev` (TypeScript, WebAssembly).

## The procedure

1. **Find the API.** Find the OpenAPI file, or read the routes. Keep the `.brev` in the API's repository.
2. **Draft.** Run `jevbrev import openapi.json > api.brev`, then `jevbrev check api.brev`. Fix every line it reports:
   - Confirm each `read?` and `change?`. Anything that writes, sends or charges is `change`.
   - Write a one-line summary per action, in the users' words.
   - For each input, write words that complete "Does `request` say ___?".
   - Use specific types: `weekday`, `time`, `date`, `one:A|B`, `ref:ACTION`.
   - Add a role group where two inputs take the same kind of word.
   - Repeat until `check` prints `ok`. Show the person the file.
3. **Label before asking.** Write 20 to 40 JSONL labels (`request`, `action`, `inputs` with `null` for not given, `known`). Hold out a quarter.
4. **Calibrate.** Say how many calls you will make (about $0.00003 each). For each label:
   - Send `jevbrev menu` and read it with `jevbrev action`.
   - Send `jevbrev frames … --known`. If its `questions` is empty, don't send it; read with `{}`.
   - Read the answers with `jevbrev read`.

   Score action, presence and value. Fix **words**, not code. Run the held-out labels once at the end. The key stays in the environment, and is never printed or written.
5. **Wire in.** Follow using-brev.md section 3.
   - `known` takes a string for a value the app already has, or a list for choices.
   - `unsure` means ask the person. `unasked` means retry. Neither is `unsaid`.
   - The app normalises values and resolves `ref` IDs.
   - Every `change` previews, and runs only on confirmation.
6. **Test without Jev.** Store measured answers as fixtures, compare against stored output, and run `jevbrev check` in CI.
7. **When the API changes:** run `jevbrev import openapi.json --into api.brev > api.new.brev`. Read the report, write words for what was added, check, and recalibrate changed actions.

## Report to the person

The calibration table (tuning and held-out, with counts, cost and number of calls), every miss with its sentence, every call you made that they didn't, and what was not measured.

## Never

- Call Jev from an automated test.
- Score a failed call as "not said".
- Skip a `change` preview because Jev was confident.
- Put a key or secret in a `.brev` file.
- Send real users' text to Jev before checking that this is allowed for their data. Requests go to TypeSafe (or OpenRouter).
- Treat a Jev answer as a permission check. The API checks access.
