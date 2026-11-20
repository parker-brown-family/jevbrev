# Using a .brev file

A `.brev` file describes an HTTP API in the words a person would use to ask for each action. You write it once and keep it beside the API it describes. Code then uses it, on every request, to do three things:

1. Write the questions Jev (TypeSafe's System One model) answers about the person's request.
2. Read Jev's answers into the action, the inputs she said, the inputs she did not, and their values.
3. Build the HTTP call. A language model can also call the API from the short text or tool definitions jevbrev prints.

Jev never reads the `.brev` file itself. It answers short, single-fact questions that jevbrev writes from the file. This is the TypeSafe pattern of building questions from a spec, described in the [Function calling](https://docs.typesafe.ai/cookbooks/function_calling) cookbook, applied to HTTP APIs. jevbrev never calls Jev and never holds a key: it prints request bodies and reads answers, and your code sends them.

Every JSON block below is real jevbrev output, or real input to it. Jev answers are marked **measured** (from real calls) or **illustrative** (written to show a reading rule).

Read these TypeSafe pages first. This document assumes them:

| Page | What this document relies on |
|---|---|
| [API reference](https://docs.typesafe.ai/api) | request body (`state`, `model`, `questions`), Noul and Choice answers |
| [Choice](https://docs.typesafe.ai/primitives/choice) | the action menu, role questions and picks |
| [Noul](https://docs.typesafe.ai/primitives/noul) | "did she say it" questions |
| [State](https://docs.typesafe.ai/concepts/state) | why the request and the candidates go in `state`, not in the question |
| [Pre-parsed value extraction](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook) | code finds candidate values, Jev picks, code copies the value verbatim |
| [Parallel questions](https://docs.typesafe.ai/cookbooks/parallel_questions) | why all questions about one request go in one call |
| [JavaScript SDK](https://docs.typesafe.ai/sdk/javascript) | `client.systemOne(body)`, used in the TypeScript examples |

## 1. The file

The complete example is [`examples/clinic.brev`](../examples/clinic.brev). This is one action from it:

```text
api Example clinic bookings  https://clinic.example/api
request  what the practitioner typed

roles schedule
  now                 the {kind} the appointments are on now, before the move
  moves_to            the {kind} the appointments should move to
  stays=now+moves_to  the {kind} the appointments are on now and stay on after the move
  neither             a {kind} from the past, another person, or not about these appointments

change move_client_series  POST /clients/{client}/bookings/move
  Move a client's recurring appointments from one weekday and time to another, from a date onward
  client        ref:find_client                which client's appointments move
  from_weekday  weekday  schedule.now          which weekday the appointments are on now
  from_time     time     schedule.now          what time the appointments are at now
  to_weekday    weekday  schedule.moves_to     which weekday the appointments should move to
  to_time       time     schedule.moves_to     what time the appointments should move to
  starting      date                           when the move starts: a date, a week or a month
  count?        int                            how many appointments to move
  yes shift Priya's Wednesday 10:00s to Friday at 11:00 from next month
  no  Sam used to come Thursdays at 5
```

## 2. Syntax

The file is UTF-8 text, read one line at a time. A line starting with `#` is a comment. A blank line ends a block. Indented lines start with spaces; a tab is an error. Columns are separated by one or more spaces. `WORDS` is always the rest of the line.

| Line | Form | Meaning |
|---|---|---|
| API | `api NAME  BASE_URL` | Required, once. `NAME` may contain spaces; `BASE_URL` is the last column and starts with `http://` or `https://`. |
| Request | `request WORDS` | What the `request` field in `state` holds. Documentation only; Jev never sees it. |
| Role group | `roles GROUP` | Starts a group of roles that compete for the same words. At least two roles. |
| Role | `  ROLE WORDS` or `  ROLE=A+B WORDS` | One role. `{kind}` in `WORDS` becomes the candidate's type noun (`weekday`, `time`, `date`). `=A+B` means this role also fills roles `A` and `B`. |
| Action | `read NAME  METHOD PATH` or `change NAME  METHOD PATH` | `read` has no side effects. `change` must return a preview and run only after the person confirms. `METHOD` is in capitals. `PATH` starts with `/`; each `{name}` in it is an input. |
| Summary | `  WORDS` | The first indented line after an action. One line, in the person's words, never cut short. |
| Input | `  NAME[?]  TYPE  [GROUP.ROLE]  WORDS` | `?` marks it optional. `WORDS` complete the sentence "Does `request` say ___?" |
| Example | `  yes WORDS` / `  no WORDS` | Sentences that do and do not ask for this action. For calibration runs by hand only. |
| Unwritten | `?` in place of `WORDS` or a summary | Written by `jevbrev import`. `check` fails until it is replaced. |
| Guess | `read? ...` / `change? ...` | Written by `jevbrev import` from the HTTP method. `check` fails until it is confirmed. |

`NAME`s are identifiers: a letter or `_`, then letters, digits or `_`. They are the API's own field names, so they are not renamed. `yes`, `no` and `none_of_these` are reserved.

`jevbrev check` also refuses: a `{name}` in a path that is not an input of its action, or one marked optional; a `GROUP.ROLE` that is not declared; a role input whose type has no spotter; a `ref:ACTION` that does not name a `read` action in the file; words that only repeat the input's name; a summary ending in `...`; a duplicate name.

### Types

| Type | How candidates are found | Value returned |
|---|---|---|
| `weekday` | day names: `Monday`, `mondays` | the day name, without a plural `s` |
| `time` | `9`, `9:30`, `9.30`, `9am`, `4 pm`, and plurals such as `9:30s`; never inside a date or a longer number | the time as written, without a plural `s` |
| `date` | `2026-12-01`, `12/1`, month names (`Dec 3rd`), ordinals (`the 8th`), `next week`, `this month`, `next Friday`, `today`, `tomorrow` | the span as written; the caller turns it into a date |
| `one:A\|B\|C` | the listed values | one of the listed values |
| `ref:ACTION` | the caller passes them (`known`, below) | one of the values passed |
| `str`, `int` | none | none, unless the caller passes `known` values |

A spotter over-finds on purpose. Jev decides which spans matter. jevbrev never invents or rewrites a value: it returns a span of the request, a listed value, or a value the caller passed.

## 3. Using it in code: two Jev calls per request

```
request ──► call 1: which action ──► call 2: every input question for that action ──► read ──► HTTP call, or a question back to her
```

The second call waits for the first, because the chosen action decides which questions exist. Within a call, every question about the request goes together ([Parallel questions](https://docs.typesafe.ai/cookbooks/parallel_questions)). The rule is to use a second call only when the first answer decides what it asks. Asking every action's questions in one call, as TypeSafe's function-calling cookbook does, has not been measured here yet ([issue 1](https://github.com/parker-brown-family/jevbrev/issues/1)). If it matches the two-call results, it becomes the default.

The same operations exist in three places:

| | Rust crate `jevbrev` | npm package `@parker-brown-family/jevbrev` (WebAssembly) | Command `jevbrev` |
|---|---|---|---|
| load and check | `jevbrev::load(text)` | `Brev.load(text)` (throws) | `check FILE` |
| call 1 | `menu(&brev, request, context)` | `brev.menu(request, context?)` | `menu FILE REQUEST [--context JSON]` |
| read call 1 | `read_action(&brev, &answers)` | `brev.readAction(answers)` | `action FILE ANSWERS.json` |
| call 2 | `frames(&brev, action, request, known, context)` | `brev.frames(action, request, known?, context?)` | `frames FILE ACTION REQUEST [--known JSON] [--context JSON]` |
| read call 2 | `read(&brev, action, &call2, &answers)` | `brev.read(action, call2, answers)` | `read FILE ACTION CALL2.json ANSWERS.json` |
| HTTP call | `http_call(&brev, &read)` | `brev.httpCall(read)` (throws if not complete) | `http FILE READ.json` |
| what to ask her | `ask_for(&brev, &read)` | `brev.askFor(read)` | `ask FILE READ.json` |

`context` is an optional object whose fields go into `state` beside `request`, such as who is signed in. A JSON file argument to the command may be `-` for stdin.

### Step 1: the action menu

```ts
import { Brev } from "@parker-brown-family/jevbrev";
import { TypeSafeClient } from "@typesafe-ai/sdk";

const brev = Brev.load(await readFile("clinic.brev", "utf8"));
const request = "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month";
const call1 = brev.menu(request);
```

`call1` is a complete `/v1/systemone` body:

```json
{
  "model": "jev-latest",
  "state": {
    "request": "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month"
  },
  "questions": {
    "action": {
      "type": "choice",
      "instructions": "Which action would carry out `request`? Choose none_of_these when no action does what `request` asks.",
      "criteria": {
        "find_client": "Find the clinic's clients by part of their name",
        "list_bookings": "List bookings for a day, or one client's upcoming bookings",
        "move_client_series": "Move a client's recurring appointments from one weekday and time to another, from a date onward",
        "cancel_booking": "Cancel one booking and tell the client",
        "none_of_these": "No action here does what `request` asks."
      }
    }
  }
}
```

Each option is the action's summary, word for word.

### Step 2: send it and read the action

```ts
const client = new TypeSafeClient();                  // or any endpoint serving /v1/systemone
const pick = brev.readAction(await client.systemOne(call1));
```

`readAction` accepts the whole response or just its `answers`, and returns one of:

| Answer | Returned |
|---|---|
| top option is an action, p ≥ 0.5 | `{ "action": "move_client_series", "p": 1.0 }` |
| top option is `none_of_these`, p ≥ 0.5 | `{ "action": "none_of_these", "p": 0.97 }`. Do not call the API. |
| top p < 0.5 | `{ "unsure": true, "p": 0.45 }`. Ask the person what they want. |
| no `action` answer, or an option that is not in the file | `{ "unasked": true }`. Retry or fail. This is not `none_of_these`. |

### Step 3: the input questions

```ts
const call2 = brev.frames("move_client_series", request, {
  client: ["Priya Shah", "Sam Ortiz", "Jordan Lee"],
});
```

`known` (the third argument) tells jevbrev what the caller already knows about an input:

- **a list of strings**: the values the input may take, such as the clinic's client names. Jev picks one.
- **one string**: the value itself, such as the client on screen. Nothing is asked, and the value is recorded in `state.given`.

jevbrev writes three kinds of question:

| Input | Question id | Question |
|---|---|---|
| in a role group | `role.N`, one per candidate span (`role.GROUP.N` when an action uses two groups) | a Choice over the group's roles |
| not in a group, not given | `said.NAME` | a Noul: "Does `request` say WORDS?" |
| not in a group, not given, with candidates (spotted, listed by `one:`, or passed in `known`) | `pick.NAME` | a Choice over the candidates, plus `none_of_these` |

For the request above, `call2` is (two `role.N` questions shortened here; jevbrev prints all four in full):

```json
{
  "model": "jev-latest",
  "state": {
    "request": "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month",
    "candidates": ["Wednesday", "Friday", "10:00", "11:00"],
    "client_candidates": ["Priya Shah", "Sam Ortiz", "Jordan Lee"],
    "starting_candidates": ["Wednesday", "Friday", "next month"]
  },
  "questions": {
    "role.0": {
      "type": "choice",
      "instructions": "In `request`, what is \"Wednesday\" (`candidates[0]`)?",
      "criteria": {
        "now": "the weekday the appointments are on now, before the move",
        "moves_to": "the weekday the appointments should move to",
        "stays": "the weekday the appointments are on now and stay on after the move",
        "neither": "a weekday from the past, another person, or not about these appointments"
      }
    },
    "role.2": {
      "type": "choice",
      "instructions": "In `request`, what is \"10:00\" (`candidates[2]`)?",
      "criteria": {
        "now": "the time the appointments are on now, before the move",
        "moves_to": "the time the appointments should move to",
        "stays": "the time the appointments are on now and stay on after the move",
        "neither": "a time from the past, another person, or not about these appointments"
      }
    },
    "said.client": { "type": "noul", "instructions": "Does `request` say which client's appointments move?" },
    "pick.client": {
      "type": "choice",
      "instructions": "Which of `client_candidates` is which client's appointments move?",
      "criteria": { "Priya Shah": null, "Sam Ortiz": null, "Jordan Lee": null, "none_of_these": "`request` does not say this." }
    },
    "said.starting": { "type": "noul", "instructions": "Does `request` say when the move starts: a date, a week or a month?" },
    "pick.starting": {
      "type": "choice",
      "instructions": "Which of `starting_candidates` is when the move starts: a date, a week or a month?",
      "criteria": { "Wednesday": null, "Friday": null, "next month": null, "none_of_these": "`request` does not say this." }
    },
    "said.count": { "type": "noul", "instructions": "Does `request` say how many appointments to move?" }
  }
}
```

Noul questions carry no `criteria` field. It is optional in the API reference, the measured questions had none, and one endpoint has rejected Noul `criteria` with a 400.

**If `questions` is empty, do not send it.** That happens when every input was given. Jev rejects a request with no questions. Call `read` with `{}` as the answers.

### Step 4: send it and read the inputs

```ts
const answers2 = await client.systemOne(call2);
const read = brev.read("move_client_series", call2, answers2);
```

`read` takes the call 2 body as well as the answers. It re-finds the candidates in `call2.state.request` and refuses a body whose candidates differ, so answers are never read against the wrong spans.

The role answers here are **measured**; the `said.client`, `pick.*` and `said.count` answers are **illustrative**:

```json
"role.0": { "type": "choice", "choice": "now",      "probabilities": { "now": 1.0,  "moves_to": 0.0, "stays": 0.0, "neither": 0.0 } },
"role.1": { "type": "choice", "choice": "moves_to", "probabilities": { "now": 0.0,  "moves_to": 1.0, "stays": 0.0, "neither": 0.0 } },
"role.2": { "type": "choice", "choice": "now",      "probabilities": { "now": 0.99, "moves_to": 0.0, "stays": 0.0, "neither": 0.01 } },
"role.3": { "type": "choice", "choice": "moves_to", "probabilities": { "now": 0.0,  "moves_to": 1.0, "stays": 0.0, "neither": 0.0 } },
"said.starting": { "type": "noul", "noul": 0.92 }
```

`read` returns every input, in file order, in one of four states:

```json
{
  "action": "move_client_series",
  "inputs": {
    "client":       { "state": "said",   "value": "Priya Shah", "p": 0.97 },
    "from_weekday": { "state": "said",   "value": "Wednesday",  "p": 1.0 },
    "from_time":    { "state": "said",   "value": "10:00",      "p": 0.99 },
    "to_weekday":   { "state": "said",   "value": "Friday",     "p": 1.0 },
    "to_time":      { "state": "said",   "value": "11:00",      "p": 1.0 },
    "starting":     { "state": "said",   "value": "next month", "p": 0.9 },
    "count":        { "state": "unsaid", "p": 0.03 }
  },
  "complete": true
}
```

| State | Meaning | What the caller does |
|---|---|---|
| `said` | The request says it. `value` is a span of the request, a `one:` value, or a `known` value. `value` is absent when there was nothing to read it from (a `str` or `int` input with no `known` values). | Use the value. |
| `unsaid` | The request does not say it. | Required: ask her. Optional: leave it out, and the API's default applies. |
| `unsure` | Jev's answer was not clear enough, or two different values claim it (listed in `claims`). | Ask her. Never guess. |
| `unasked` | No answer came back for its question. | Retry the call, or fail. Never treat it as `unsaid`. |

`p` is the probability behind the state, and is absent when no single answer decided it. `complete` is `true` only when every required input is `said` **and** has a value.

### Reading rules

These are exact, so the crate, the npm package and the tests agree.

1. An answer counts when its top option has p ≥ 0.5 (`THRESHOLD`).
2. A Noul at p ≥ 0.5 is said; below it is unsaid.
3. A role answer below the threshold claims nothing. A role with `=A+B` claims for itself and for `A` and `B`.
4. Each grouped input collects the spans whose role claims it and whose type matches its own. Spans are compared after lower-casing and dropping a plural `s` and a whole-hour `:00`, so two `Monday`s are one value. One value: `said`. None: `unsaid`. Two or more: `unsure`, with `claims`.
5. If any role answer in a group is missing, every input in that group is `unasked`.
6. For an input with a pick: said and a confident pick of a candidate gives `said` with that value (p is the lower of the two). Unsaid and a confident `none_of_these`, or unsaid and an unconfident pick, gives `unsaid`. Any other combination gives `unsure`. A missing pick answer gives `unasked`.
7. A given value is `said` with no `p`.
8. `value` is always copied as found. Normalising it (`next month` to a date, `10:00` to `10:00:00`) and resolving a `ref` to its ID are the caller's job.

Rule 4's merge matters. In "Sam used to come Thursdays at 5, but move his current Monday 9:30s to Monday 8:30", both `Monday`s are `stays` (**measured**: 0.73 and 0.60). Without the merge, two equal spans read as a conflict and both weekday inputs come back `unsure`.

### Step 5: the HTTP call, or the question back

```ts
if (read.complete) {
  const http = brev.httpCall(read);
} else {
  const ask = brev.askFor(read);
}
```

`httpCall` returns:

```json
{
  "method": "POST",
  "base_url": "https://clinic.example/api",
  "path": "/clients/{client}/bookings/move",
  "path_values": { "client": "Priya Shah" },
  "change": true,
  "body": {
    "from_weekday": "Wednesday",
    "from_time": "10:00",
    "to_weekday": "Friday",
    "to_time": "11:00",
    "starting": "next month"
  }
}
```

Inputs that are not in the path go to `query` for `GET`, `DELETE`, `HEAD` and `OPTIONS`, and to `body` otherwise. Only `said` inputs with values are included. Before sending: normalise the values, resolve each `ref` value to its ID with the referenced `read` action, and fill the path. `change: true` means the API returns a preview: show it, and confirm only after the person agrees.

`askFor` returns the first required input, in file order, that is `unsaid`, `unsure`, or said without a value, or `null`. It skips `unasked` inputs, because those need a retry, not a question:

```json
{ "input": "client", "state": "said", "text": "I need to know which client's appointments move." }
```

Your chat layer may replace `text`.

## 4. For a language model

`jevbrev llm FILE` prints a short text a model reads to call the API:

```text
Example clinic bookings · https://clinic.example/api · change actions return a preview; confirm with the person before running

move_client_series  POST /clients/{client}/bookings/move  change
  Move a client's recurring appointments from one weekday and time to another, from a date onward
  client: id of a find_client result — which client's appointments move
  from_weekday: weekday — which weekday the appointments are on now
```

`jevbrev tools FILE --style anthropic|openai|mcp` prints the actions as tool definitions with JSON Schema inputs. `jevbrev openapi FILE` prints OpenAPI 3.1 for documentation tools, with the words, types and roles kept under `x-jevbrev`.

## 5. Starting from OpenAPI, and keeping up with it

```sh
jevbrev import openapi.json > api.brev
jevbrev check api.brev
```

`import` reads OpenAPI 3 as JSON (convert YAML first, for example with `yq -o=json`). It carries across the method, path, path and query parameters, JSON request body properties, plain types, `enum`s as `one:` types, and each summary whole. An input with no description gets `?`. Every action is marked `read?` (GET) or `change?` (anything else). Header and cookie parameters are skipped with a comment. `check` then lists everything left to write.

When the API changes:

```sh
jevbrev import openapi.json --into api.brev > api.new.brev
```

The API decides which actions and inputs exist, each action's method and path, and whether an input is required. The existing file keeps everything a person decided: words, summaries, confirmed read or change, types, roles, role groups and examples. New actions and inputs arrive as `?`. Removed ones are listed in comments at the end of the file, words and all. The report on stderr lists `added`, `removed`, `moved` (method or path) and `requirement_changed`. Comments at the top and bottom of the file are kept. Comments inside blocks are not, because the file is rewritten in the standard layout.

`jevbrev fmt api.brev` prints a file in that standard layout, with aligned columns.

## 6. Writing the words

- **An input's WORDS complete "Does `request` say ___?"** Use an indirect question in the person's words: `which weekday the appointments should move to`. Never the parameter name, and never "the destination weekday". TypeSafe makes the same point: a question named after its parameter "gives the command nothing to match against" ([Function calling](https://docs.typesafe.ai/cookbooks/function_calling)).
- **One fact per input.** "which weekday and time they move to" is two inputs.
- **The summary is the menu option.** One complete line saying what the action does, and to what.
- **Use a role group when two or more inputs can take the same kind of word.** "Wednesday" can be where the appointments are now or where they move to. Include a role for a word that fills both (`stays=now+moves_to`) and one for a word that is about something else (`neither`).
- **`yes` and `no` examples are for calibration by hand.** Automated tests never call Jev. They compare jevbrev's output against stored files.

## 7. Measured

On 2026-11-20 the same three studies that motivated jevbrev were re-run from a 16-action `.brev` file for a practice-management API, with jevbrev writing every request and reading every answer (`jev-latest`, 56 requests, 0 failures):

| Job | Study before jevbrev | Through jevbrev | Mean cost a call |
|---|---|---|---|
| Which action (36 labelled requests) | 36 / 36 | **36 / 36** | $0.000035 |
| Did she say every required input (12) | 12 / 12 | **12 / 12** | $0.000027 |
| Values read for a move (22, in 8 held-out sentences) | 19 / 22 | **22 / 22** | $0.000035 |
| Presence of each move input (40) | 36 / 40 | **38 / 40** | |

The three values the earlier study missed were all code errors: a time spotter that cut `9:30s` to `9`, and two agreeing `Monday`s read as a conflict. Both are fixed here, and tested without Jev. Of the two presence misses, one is arguably right: in "her Monday at 3 is moving to Tuesday, same time", jevbrev reads the new time as `3`, where the label said the time was not given.

**Not measured yet:** `pick.NAME` for `ref` inputs with long lists of known values; one call for all actions at once, as the TypeSafe function-calling cookbook does; Score questions and images, which jevbrev does not use.

## Credit

jevbrev builds on the research in Daniel Garcia, "LAPIS: Lightweight API Specification for Intelligent Systems", arXiv 2602.18541 (2026). It shares no text or code with LAPIS.
