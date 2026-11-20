# jevbrev

**jevbrev lets an app understand what a person types and turn it into a call to its own API.**

Someone types *"move Priya's Wednesday 10:00s to Friday at 11:00 from next month"*. jevbrev works out that this means **move a client's recurring appointments**, from **Wednesday 10:00** to **Friday 11:00**, starting **next month**. Then it hands your app the exact API call to make. If something is missing, it hands back the question to ask instead: *"I need to know what time the appointments should move to."*

It does this by asking [Jev](https://docs.typesafe.ai), a small AI model from TypeSafe that answers typed questions with probabilities. jevbrev doesn't send Jev your whole API and hope. It asks short, simple questions it writes from a short description of your API, then reads the answers carefully.

## How it works

1. **You describe your API once**, in a short `.brev` file, in the words your users would use. If you already have an OpenAPI file, jevbrev writes the first draft for you.
2. **When someone types a request**, jevbrev asks Jev two things: which action do they want, and what did they say for each part of it.
3. **jevbrev reads Jev's answers.** Each input comes back as said (with its value), not said, unsure, or not answered. Nothing is guessed. A value is always copied from what the person typed, or from a list you provided.
4. **Your app gets the API call to make**, or the question to ask the person. Anything that changes data shows a preview first and waits for the person to confirm.

## Does it work?

We tested it on a real booking API with 16 actions, against sentences labelled by hand before any test ran:

| | Right |
|---|---|
| Picking the action someone meant | 36 of 36 |
| Knowing whether they gave everything needed | 12 of 12 |
| Reading out the days and times | 22 of 22 |

Each question costs about three thousandths of a cent. These numbers come from one API and one set of sentences. Measure your own; [the guide](docs/getting-started.md) shows how.

---

## A little more detail

### Install

| For | Run |
|---|---|
| The command line | `cargo install jevbrev` |
| Rust | `cargo add jevbrev` |
| Node and TypeScript | `npm install @parker-brown-family/jevbrev` |

The npm package is the same Rust code, compiled to WebAssembly.

### A `.brev` file

```text
api Example clinic bookings  https://clinic.example/api

change move_client_series  POST /clients/{client}/bookings/move
  Move a client's recurring appointments from one weekday and time to another, from a date onward
  client        ref:find_client                which client's appointments move
  from_weekday  weekday  schedule.now          which weekday the appointments are on now
  to_weekday    weekday  schedule.moves_to     which weekday the appointments should move to
  starting      date                           when the move starts: a date, a week or a month
```

Each action has a one-line summary. Each input has a type and the words that finish the sentence "Does the request say ___?". The complete example is [examples/clinic.brev](examples/clinic.brev).

### In TypeScript

```ts
import { Brev } from "@parker-brown-family/jevbrev";
import { TypeSafeClient } from "@typesafe-ai/sdk";

const brev = Brev.load(await readFile("clinic.brev", "utf8"));
const jev = new TypeSafeClient();
const request = "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month";

const pick = brev.readAction(await jev.systemOne(brev.menu(request)));
if ("action" in pick && pick.action !== "none_of_these") {
  const call2 = brev.frames(pick.action, request, { client: ["Priya Shah", "Sam Ortiz"] });
  const read = brev.read(pick.action, call2, await jev.systemOne(call2));
  const next = read.complete ? brev.httpCall(read) : brev.askFor(read);
}
```

### On the command line

| Command | Does |
|---|---|
| `jevbrev import openapi.json > api.brev` | writes a first draft from OpenAPI |
| `jevbrev import openapi.json --into api.brev` | updates an existing file after the API changes, keeping its words |
| `jevbrev check api.brev` | lists everything still missing, with line numbers |
| `jevbrev menu` · `frames` | print the two Jev requests |
| `jevbrev action` · `read` | read Jev's answers |
| `jevbrev http` · `ask` | the API call to make, or the question to ask |
| `jevbrev llm` · `tools` · `openapi` | the same API for a language model, as tool definitions, or as OpenAPI |

### Read next

- [Getting started](docs/getting-started.md): from "we have an API" to calibrated, step by step.
- [Using a .brev file](docs/using-brev.md): the full specification, with every request and answer shown.

---

## For agents

Agents are expected to do most of this work: drafting the file, writing the words, calibrating, and wiring it in. A person reads the words and the scores, and decides what ships.

### Read in this order

1. [docs/agents.md](docs/agents.md): the whole procedure, the calibration loop, and what to report.
2. [docs/using-brev.md](docs/using-brev.md): the specification. Its reading rules are exact.
3. [examples/clinic.brev](examples/clinic.brev): a complete file.
4. TypeSafe's [API reference](https://docs.typesafe.ai/api) and [Function calling cookbook](https://docs.typesafe.ai/cookbooks/function_calling), for what Jev does with each question.

### The skill

[`skills/jevbrev/SKILL.md`](skills/jevbrev/SKILL.md) is an agent skill in the standard `SKILL.md` format. Link or copy the `skills/jevbrev` folder into your agent's skills directory (for Claude Code, `~/.claude/skills/jevbrev`).

### The procedure, in one breath

Import or write the `.brev` file, then fix everything `check` reports. Label 20 to 40 sentences before asking Jev anything. Calibrate with `menu`, `action`, `frames` and `read`, and change the words, not the code, until the scores hold, including on held-out sentences. Wire it in following the specification. Test with stored answers. Re-import with `--into` when the API changes.

### Security

- jevbrev makes no network calls and never reads a key. Your code sends its requests to Jev.
- Each request to Jev carries the person's sentence and any values you pass, such as client names. They go to TypeSafe, or to OpenRouter and TypeSafe. Check this is allowed for your data first.
- Jev's answer is not a permission check. Your API must still check access on every call.
- Typed text is untrusted. jevbrev only lets Jev choose among the actions in your file, and returns only words from the request or values you provided. A request can still point at the wrong action, so every change previews and waits for confirmation.
- Keep secrets out of `.brev` files.
- To report a vulnerability, see [SECURITY.md](SECURITY.md).

### Disclaimers

jevbrev is an independent open-source project. It is not made, endorsed or supported by TypeSafe. Jev's answers are probabilities and can be wrong. Don't let an assistant built on it make medical, legal or financial decisions without a person confirming each one. The software is provided as is, without warranty.

---

## Develop

```sh
cargo test                                   # unit and golden tests; none call Jev
scripts/build-npm.sh                         # builds npm/ and checks it matches the crate exactly
JEVBREV_BLESS=1 cargo test --test golden     # rewrite the expected files after an intended change
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Credit

jevbrev builds on the research in Daniel Garcia, "LAPIS: Lightweight API Specification for Intelligent Systems", arXiv 2602.18541 (2026). It shares no text or code with LAPIS.

## Licence

Either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
