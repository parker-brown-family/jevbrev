# Getting started: describing your API

This is the path from "we have an API" to "our app understands what people ask for". It takes an afternoon for a small API. An agent can do most of it, and [docs/agents.md](agents.md) tells it how. A person should still read the words and the scores.

## 1. Install

```sh
cargo install jevbrev
```

## 2. Start a file

If your API has an OpenAPI file (JSON):

```sh
jevbrev import openapi.json > api.brev
```

That copies every action and input across, with each summary whole. If your API has no OpenAPI file, start from [`examples/clinic.brev`](../examples/clinic.brev) and write one action at a time.

## 3. Fill in what only a person knows

```sh
jevbrev check api.brev
```

`check` lists every line that still needs something, with its line number. Usually that is:

- **Read or change?** Each action starts `read?` or `change?`. Remove the `?` once you are sure. A `change` must show a preview and wait for the person to confirm.
- **A summary.** One line under each action, in your users' words: "Move a client's recurring appointments to another day and time". Jev picks the action from these lines, so make them plain and complete.
- **Words for each input.** Replace each `?` with words that finish the sentence "Does the request say ___?". For example: `which weekday the appointments should move to`. Never just repeat the field name.
- **Better types.** Change `str` to `weekday`, `time`, `date`, `one:keep|reschedule`, or `ref:find_client` where that is what the input is. These let jevbrev read the value out of what the person typed.
- **Role groups**, when two inputs take the same kind of word, such as the day appointments are on now and the day they move to. Copy the `roles schedule` block from the example.

Run `check` again until it says `ok`.

## 4. Write down what people will say

Before asking Jev anything, write 20 to 40 sentences your users might type. For each, write the right answer: which action, and which inputs it gives. Put a few aside and don't look at them until the end. They tell you whether your words work on sentences you didn't tune them for.

## 5. Calibrate

For each sentence, ask Jev with jevbrev's questions and compare against your answers. You need a Jev key (from [TypeSafe](https://typesafe.ai), or through OpenRouter). [docs/agents.md](agents.md#5-calibrate) has the exact loop. When something comes out wrong, change the **words**, not the code, and run again. Finish with the sentences you put aside.

## 6. Use it in your app

Follow the five steps in [docs/using-brev.md](using-brev.md#3-using-it-in-code-two-jev-calls-per-request): ask which action, ask about the inputs, read the answers, then either call your API or ask the person for what is missing.

## 7. When your API changes

```sh
jevbrev import openapi.json --into api.brev > api.new.brev
```

This keeps every word you wrote, adds new actions and inputs as `?`, and lists anything the API dropped at the end of the file. Read the report it prints, run `check`, then replace the old file. Re-run your calibration if you changed any words.
