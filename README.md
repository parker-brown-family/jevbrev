# jevbrev

**jevbrev condenses an API definition into a short `.brev` file, then turns that one file into what a machine needs to act on the API: the questions to ask Jev, the code that reads Jev's answers, the HTTP call to make, and compact definitions for language models.**

A `.brev` file gives each action of an HTTP API one line saying what it does, and each input one line saying what it is, in the words a person would use. jevbrev drafts the file from OpenAPI, lists what is still missing, and keeps the file in step when the API changes. You keep it beside the API and review it like code.

From that one file, jevbrev produces:

| Output | Used by |
|---|---|
| Two requests for [Jev](https://docs.typesafe.ai): which action a person's request wants, and what it says for each input | an app that takes typed requests |
| A reader for Jev's answers, giving each input's value or saying it is missing | the same app |
| The HTTP call to make, or the question to ask the person when something is missing | the same app |
| A compact text description, tool definitions (Anthropic, OpenAI, MCP) and OpenAPI 3.1 | language models and documentation tools |

Jev is a small model from TypeSafe that answers typed questions (pick one of these, is this true) and says how sure it is. jevbrev never calls Jev or holds a key. It writes the request bodies and reads the answers; your code sends them.

## Why it exists

**API definitions are written for code generators, so most of their size is schema.** A model choosing which action to call does not need nested objects, references and error shapes, and it pays for every token of them. On a real 16-action API, Jev picked the right action for 36 of 36 labelled requests in each of three tests: one-line summaries cost 829 tokens a call, the same API as OpenAPI cost 3,107, and as [LAPIS](https://arxiv.org/abs/2602.18541) 1,245. The summaries were as accurate and less than a third of the size.

**Jev answers best when code asks it short questions, one fact each.** On the same API, one combined question asking whether a request gave every required input was right 8 to 10 times out of 12. One question per input, each written by code from the definition ("Does the request say which day the order should arrive?"), was right 12 of 12, and named the input that was missing. Those questions need words: what each input means, as a person would say it. No API format carries them. OpenAPI could, with extensions, but in a sample action the words sat on 20 extension lines threaded through 21 lines of schema, and no OpenAPI tool reads them. A `.brev` file is mostly those words.

## What it does to the work

- **One small file is the source.** Everything above is generated from it, so nothing drifts between the Jev questions, the tool definitions and the OpenAPI.
- **A person reviews the words and nothing else.** The example shop API below is 29 lines.
- **Starting is a command.** `jevbrev import openapi.json` writes a draft with every action and input, and `jevbrev check` lists each line that still needs words, by line number. Run `check` in CI.
- **API changes keep the words.** `jevbrev import openapi.json --into api.brev` adds what is new, lists what was removed, and keeps every word a person wrote.
- **Fixing a wrong answer means changing words, not code.** If Jev picks the wrong action, the summaries are too alike. If it misses an input, that input's words are unclear.
- **Tests never call Jev.** Store real answers once, and compare jevbrev's output against stored files.

## What it costs

| | Measured |
|---|---|
| One request through Jev: both calls, 7 inputs, 9 questions | $0.000065 (2026-11-20), about 15,000 requests for a dollar |
| Mean cost of one Jev call in the 16-action study | $0.000027 to $0.000035 |
| Choosing an action from `.brev` summaries, against the same API as OpenAPI | 829 tokens a call against 3,107, with the same 36 of 36 right |

For a language model, `jevbrev llm` prints the example shop API in 879 bytes. Its tool definitions take 2,186 bytes (Anthropic) or 2,528 (OpenAI), and its OpenAPI 5,735. All four come from the same file, so they describe the same API.

## Does it work?

On the 16-action API, against requests labelled by hand before any test ran:

| | Right |
|---|---|
| Picking the action the request meant | 36 of 36 |
| Knowing whether the request gave every required input | 12 of 12 |
| Reading out the values (days, times, dates) | 22 of 22 |

These numbers come from one API and one set of requests. Measure your own; [the guide](docs/getting-started.md) shows how.

---

## Using it

### Install

| For | Run |
|---|---|
| The command line | `cargo install jevbrev` |
| Rust | `cargo add jevbrev` |
| Node and TypeScript | `npm install @parker-brown-family/jevbrev` |

The npm package is the same Rust code, compiled to WebAssembly.

### A `.brev` file

Two of the four actions in [examples/shop.brev](examples/shop.brev):

```text
api Example shop  https://shop.example/api
request  what the customer typed

read find_orders  GET /orders
  List a customer's orders, newest first
  since?  date  how far back to look

change move_delivery  PATCH /orders/{order}/delivery
  Change the day an order is delivered
  order  ref:find_orders  which order's delivery changes
  day    date             which day the order should arrive instead
  yes can my order come on Friday instead
  no  where is my order
```

Each action is `read` or `change`, with its method and path, then a one-line summary. Each input has a name, a type, and words that finish the sentence "Does the request say ___?". A `?` after a name makes the input optional. The `yes` and `no` lines are requests for calibrating by hand. A `change` always returns a preview and waits for the person to confirm.

[examples/clinic.brev](examples/clinic.brev) is a larger example. It uses role groups, for when two inputs take the same kind of word, such as the day something is on now and the day it moves to.

### What comes back

Measured on real Jev, with that file:

| Typed | jevbrev returns |
|---|---|
| "can my order come on Friday instead", with the order on screen passed in | `PATCH /orders/o_1042/delivery` with `{"day": "Friday"}`, as a preview to confirm |
| "cancel my last order" | the question to ask: "I need to know why the order is being cancelled." |
| "I want to return the blue mug" | no action: the file has none for returns |

### In TypeScript

```ts
import { Brev } from "@parker-brown-family/jevbrev";
import { TypeSafeClient } from "@typesafe-ai/sdk";

const brev = Brev.load(await readFile("shop.brev", "utf8"));
const jev = new TypeSafeClient();

const pick = brev.readAction(await jev.systemOne(brev.menu(request)));
if ("action" in pick && pick.action !== "none_of_these") {
  const call2 = brev.frames(pick.action, request, known);   // known: values the app already has
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
| `jevbrev fmt api.brev` | prints the file in the standard layout |
| `jevbrev menu` · `frames` | print the two Jev requests |
| `jevbrev action` · `read` | read Jev's answers |
| `jevbrev http` · `ask` | the API call to make, or the question to ask |
| `jevbrev llm` · `tools` · `openapi` | the same API for a language model, as tool definitions, or as OpenAPI |

### Read next

- [Getting started](docs/getting-started.md): from "we have an API" to a calibrated `.brev`, step by step.
- [Using a .brev file](docs/using-brev.md): the full specification, with every request and answer shown.

---

## For agents

Agents are expected to do most of this work: drafting the file, writing the words, calibrating, and wiring it in. A person reads the words and the scores, and decides what ships.

### Read in this order

1. [docs/agents.md](docs/agents.md): the whole procedure, the calibration loop, and what to report.
2. [docs/using-brev.md](docs/using-brev.md): the specification. Its reading rules are exact.
3. [examples/shop.brev](examples/shop.brev) and [examples/clinic.brev](examples/clinic.brev): complete files.
4. TypeSafe's [API reference](https://docs.typesafe.ai/api) and [Function calling cookbook](https://docs.typesafe.ai/cookbooks/function_calling), for what Jev does with each question.

### The skill

[`skills/jevbrev/SKILL.md`](skills/jevbrev/SKILL.md) is an agent skill in the standard `SKILL.md` format. Link or copy the `skills/jevbrev` folder into your agent's skills directory (for Claude Code, `~/.claude/skills/jevbrev`).

### The procedure, in one breath

Import or write the `.brev` file, then fix everything `check` reports. Label 20 to 40 requests before asking Jev anything. Calibrate with `menu`, `action`, `frames` and `read`, and change the words, not the code, until the scores hold, including on held-out requests. Wire it in following the specification. Test with stored answers. Re-import with `--into` when the API changes.

### Security

- jevbrev makes no network calls and never reads a key. Your code sends its requests to Jev.
- Each request to Jev carries the person's text and any values you pass, such as customer names. They go to TypeSafe, or to OpenRouter and TypeSafe. Check this is allowed for your data first.
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
