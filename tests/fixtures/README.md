# Test fixtures

Automated tests never call Jev. They read these stored answers and compare jevbrev's output with the files in `../golden/`.

- `answers.move.json`: every answer is **measured**, from real Jev (`jev-1.14`, through OpenRouter) on 2026-11-20, for the request and candidates in `../golden/frames.move.json`.
- `answers.sam.json`: the `role.N` and `said.starting` answers are **measured**, from real Jev (`jev-latest`) on 2026-11-20 in a held-out run. Every other answer is **illustrative**: written by hand to exercise the reading rules. In that run an older time spotter offered `"9"` for `9:30s`; the answers are kept as Jev gave them.
- `shop.openapi.json`: a small made-up OpenAPI document for `import`.
- `bad.brev`: a file with one of each common mistake, for `check`.

Regenerate the golden files after an intended change with `JEVBREV_BLESS=1 cargo test --test golden`, and read the diff before committing it.
