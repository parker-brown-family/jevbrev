# For agents: how to drive jevbrev

This is for a coding agent asked to put a `.brev` file in front of an API, calibrate it on Jev, or wire it into an app. A person decides what ships. Your job is to do the work and show them the evidence.

The installable skill is [`skills/jevbrev/SKILL.md`](../skills/jevbrev/SKILL.md). It is a short version of this page.

## 1. Read, in this order

1. This page.
2. [docs/using-brev.md](using-brev.md): the specification. Every request and answer is shown. Its reading rules are exact.
3. [examples/clinic.brev](../examples/clinic.brev): a complete file.
4. TypeSafe's [API reference](https://docs.typesafe.ai/api), [Choice](https://docs.typesafe.ai/primitives/choice), [Noul](https://docs.typesafe.ai/primitives/noul) and the [Function calling](https://docs.typesafe.ai/cookbooks/function_calling) cookbook. Read these when you need to know what Jev does with a question. Any TypeSafe page serves markdown if you add `.md` to its URL.
5. `jevbrev --help`.

Do not read the Rust source to learn the format. If the specification and the code disagree, that is a bug: report it.

## 2. Find the API

Look for an OpenAPI file first (`openapi.json`, `swagger.json`, a `/openapi.json` route, or a docs build). If there is only YAML, convert it (`yq -o=json`). If there is none, read the API's route handlers and write the `.brev` by hand. The action names and input names must be the API's own field names.

Keep the `.brev` file beside the API it describes, in that API's repository.

## 3. Draft and check

```sh
jevbrev import openapi.json > api.brev
jevbrev check api.brev
```

Work through every line `check` reports:

| `check` says | You do |
|---|---|
| `read? is a guess` / `change? is a guess` | Decide from what the handler does. Anything that writes, sends or charges is `change`. |
| `has no summary` | One line, in the users' words, saying what the action does and to what. |
| `has no words` | Words that complete "Does `request` say ___?": `which weekday the appointments should move to`. |
| `words repeat its name` | Say what a person would say, not the field name. |
| `has no spotter` | A role input must be `weekday`, `time` or `date`. |

Also, without being told:

- Give inputs specific types where they fit: `weekday`, `time`, `date`, `one:A|B`, `ref:ACTION`.
- Where two or more inputs can take the same kind of word ("now" and "moves to"), add a role group with a role for each, a `stays=…` role for a word that fills both, and `neither`.
- Add two or three `yes` and `no` sentences per action.

Use `jevbrev fmt api.brev` to lay the file out tidily. When `check` prints `ok`, show the person the file. They own the words.

## 4. Label before you ask

Write the labels before any call to Jev, so the scores mean something. One JSON object per line:

```json
{"request": "move Sam's Monday 9:30s to Tuesday at 10 from next week", "action": "move_client_series", "inputs": {"from_weekday": "Monday", "from_time": "9:30", "to_weekday": "Tuesday", "to_time": "10", "starting": "next week", "count": null}, "known": {"client": "c_123"}}
```

- `action`: the right action, or `none_of_these`.
- `inputs`: the value the sentence gives for each input, or `null` when it does not give one. Leave out an input you are not scoring.
- `known`: what the app would pass to `frames` at that moment, such as the client on screen.

Write 20 to 40 labels covering every action, sentences that should match none, and the hard cases: two days in one sentence, a day from the past, "same time". Mark about a quarter `"held_out": true`, and don't look at those results until the end.

## 5. Calibrate

Calibration calls Jev, so it costs money and needs a key. Tests never call Jev. Each request costs about $0.00003. Say how many calls you will make before you make them.

The key lives in the environment (`TYPESAFE_API_KEY`, or `OPENROUTER_API_KEY` for OpenRouter). Never print it, write it to a file, or pass it on a command line that gets logged.

Write the loop yourself, in whatever the project uses. This is the whole of it, in shell:

```bash
send() {   # TypeSafe; for OpenRouter use https://openrouter.ai/api/v1/systemone and $OPENROUTER_API_KEY
  curl -sS https://api.typesafe.ai/v1/systemone \
    -H "Authorization: Bearer $TYPESAFE_API_KEY" -H 'Content-Type: application/json' --data-binary @-
}
while IFS= read -r label; do
  req=$(jq -r .request <<<"$label")
  jevbrev menu api.brev "$req" | send > ans1.json
  action=$(jevbrev action api.brev ans1.json | jq -r '.action // "unsure-or-unasked"')
  read='null'
  case "$action" in
    none_of_these|unsure-or-unasked) ;;
    *)
      jevbrev frames api.brev "$action" "$req" --known "$(jq -c '.known // {}' <<<"$label")" > call2.json
      if [ "$(jq '.questions | length' call2.json)" -gt 0 ]; then send < call2.json > ans2.json; else echo '{}' > ans2.json; fi
      read=$(jevbrev read api.brev "$action" call2.json ans2.json) ;;
  esac
  jq -nc --argjson label "$label" --arg got "$action" --argjson read "$read" \
    '{label: $label, got: $got, read: $read}' >> results.jsonl
done < labels.jsonl
```

Then score `results.jsonl`:

| Score | Counts as right |
|---|---|
| action | `got` equals `label.action` |
| presence, per labelled input | `read.inputs[name].state == "said"` exactly when the label is not `null` |
| value, per input the label gives | said, and the values match after lower-casing and dropping a plural `s` and a whole-hour `:00` |

Also count `unasked` (failed calls) separately. Never score one as `unsaid`.

When something is wrong:

- **Wrong action:** the summaries are too close, or one is vague. Rewrite the summaries.
- **Presence wrong:** the input's words are unclear, or ask two things at once. Rewrite or split the input.
- **Value wrong:** check the candidates in `call2.json`'s state first. If the right word is not there, it is a spotter or type problem: report it as a jevbrev issue. If it is there, the role words need work.

Change words, not code. Re-run until the tuning set is right, then run the held-out labels once. Report both.

## 6. Wire it into the app

Follow docs/using-brev.md section 3. The things agents get wrong:

- Load the file once at start-up (`Brev.load`). It throws on any `check` problem, which is what you want.
- Pass what the app already knows as `known`: a single string for a value (the client on screen), a list for choices (the practice's client names).
- An empty `questions` in call 2 means don't send it. Read with `{}`.
- `unsure` means ask the person. `unasked` means retry or fail. Neither is `unsaid`.
- Normalise values ("next month" to a date) and resolve `ref` values to IDs in app code. jevbrev returns words as written.
- A `change` action previews, and runs only on the person's confirmation. Never skip this because Jev was confident.

## 7. Test without Jev

Save real answers from calibration as test fixtures, marked measured. Store jevbrev's output for them as expected files, and compare. Run `jevbrev check` in CI. No automated test calls Jev.

## 8. When the API changes

```sh
jevbrev import openapi.json --into api.brev > api.new.brev
```

The report (on stderr) lists `added`, `removed`, `moved` and `requirement_changed`. Write words for everything added, decide whether removed actions' callers need changing, run `check`, and re-run calibration for any action whose words changed.

## 9. What to tell the person

- The calibration table: action, presence and value, tuning and held-out, with counts ("34 / 36"), cost and how many calls.
- Every miss, with its sentence.
- Every decision you made that they didn't: a read-or-change call, a type choice, a role group.
- What you did not measure.

## Security

- jevbrev makes no network calls and reads no keys. Your code sends its requests.
- **What leaves your system.** Each Jev request contains the person's sentence, the candidate words found in it, and any `known` values you pass, such as client names. These go to TypeSafe, or to OpenRouter and TypeSafe. Check this is allowed for your data before you send real users' requests. Pass only the `known` values the question needs.
- **Jev is not a permission check.** A read result says what the person asked for, not whether they may do it. The API must still check access on every call.
- **Text in the request is untrusted.** Someone can type anything. jevbrev only lets Jev choose among the actions in the file, and returns only words from the request, `one:` values, or `known` values, so a request cannot invent an action or a value. It can still lead to the wrong action, which is why every `change` previews and waits for confirmation.
- Keep secrets out of `.brev` files. They describe an API, and get committed and shared.

## Disclaimers

- jevbrev is an independent open-source project. It is not made, endorsed or supported by TypeSafe.
- Jev's answers are probabilities and can be wrong. The calibration numbers in this repository come from one API and one set of sentences. Measure your own.
- Do not let a `.brev`-driven assistant make medical, legal or financial decisions without a person confirming each one.
- The software is provided as is, without warranty. See the licences.
