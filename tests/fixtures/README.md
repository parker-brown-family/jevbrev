# Test fixtures

Automated tests never call Jev. They read these stored answers and compare jevbrev's output with the files in `../golden/`.

- `answers.priya.json` and `answers.sam.json`: the `role.N` and `said.starting` answers are **measured**. They come from real Jev (`jev-latest`) calls on 2026-11-20, in a held-out run of the same questions on the same sentences. Every other answer is **illustrative**: written by hand to exercise the reading rules.
- In the measured runs, an older time spotter offered `"10"` for `10:00s` and `"9"` for `9:30s`. The answers are kept as Jev gave them. Only the spans the current spotter offers differ.
- `shop.openapi.json`: a small made-up OpenAPI document for `import`.
- `bad.brev`: a file with one of each common mistake, for `check`.

Regenerate the golden files after an intended change with `JEVBREV_BLESS=1 cargo test --test golden`, and read the diff before committing it.
