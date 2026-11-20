# jevbrev

Short API definitions that code turns into [Jev](https://docs.typesafe.ai) questions, answer readers and HTTP calls.

A `.brev` file describes an HTTP API in the words a person would use to ask for each action:

```text
change move_client_series  POST /clients/{client}/bookings/move
  Move a client's recurring appointments from one weekday and time to another, from a date onward
  client        ref:find_client                which client's appointments move
  from_weekday  weekday  schedule.now          which weekday the appointments are on now
  to_weekday    weekday  schedule.moves_to     which weekday the appointments should move to
  starting      date                           when the move starts: a date, a week or a month
```

For one person's request, jevbrev writes the two Jev (TypeSafe System One) requests: one to pick the action, and one to ask about every input. It then reads Jev's answers into which inputs the request said, which it did not, and their values. Finally it builds the HTTP call, or the question to ask back. It can also print the same API as tool definitions or short text for a language model, and as OpenAPI.

jevbrev never calls Jev and never holds a key. It prints request bodies and reads answers; your code sends them.

**How to use it, step by step with every request and answer: [docs/using-brev.md](docs/using-brev.md).**

## Install

| | |
|---|---|
| Command line | `cargo install jevbrev` |
| Rust | `cargo add jevbrev` |
| Node and TypeScript (WebAssembly) | `npm install jevbrev` |

## In TypeScript

```ts
import { Brev } from "jevbrev";
import { TypeSafeClient } from "@typesafe-ai/sdk";

const brev = Brev.load(await readFile("clinic.brev", "utf8"));
const client = new TypeSafeClient();
const request = "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month";

const pick = brev.readAction(await client.systemOne(brev.menu(request)));
if ("action" in pick && pick.action !== "none_of_these") {
  const call2 = brev.frames(pick.action, request, { client: ["Priya Shah", "Sam Ortiz"] });
  const read = brev.read(pick.action, call2, await client.systemOne(call2));
  if (read.complete) {
    const http = brev.httpCall(read);   // { method, path, path_values, body, change }
  } else {
    const ask = brev.askFor(read);      // { input, state, text: "I need to know ..." }
  }
}
```

## On the command line

```sh
jevbrev check api.brev
jevbrev import openapi.json > api.brev
jevbrev menu api.brev "move Sam's Mondays to Tuesday"
jevbrev frames api.brev move_client_series "move Sam's Mondays to Tuesday" --known '{"client":["Sam Ortiz"]}'
jevbrev read api.brev move_client_series call2.json answers.json
jevbrev tools api.brev --style anthropic
```

## Measured

Re-running three studies on Jev from a 16-action `.brev` file, with jevbrev writing every request and reading every answer (`jev-latest`, 2026-11-20, 56 requests):

| Job | Right | Mean cost a call |
|---|---|---|
| Which action, from 36 labelled requests | 36 / 36 | $0.000035 |
| Did the request give every required input, 12 labelled | 12 / 12 | $0.000027 |
| Values read for a move, in 8 held-out sentences | 22 / 22 | $0.000035 |

The details, and what is not measured yet, are in [docs/using-brev.md](docs/using-brev.md#7-measured).

## Develop

```sh
cargo test                     # unit and golden tests; none call Jev
scripts/build-npm.sh           # builds npm/ and checks it prints exactly what the crate prints
JEVBREV_BLESS=1 cargo test --test golden   # rewrite the golden files after an intended change
```

## Credit

jevbrev builds on the research in Daniel Garcia, "LAPIS: Lightweight API Specification for Intelligent Systems", arXiv 2602.18541 (2026). It shares no text or code with LAPIS.

## Licence

Either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
