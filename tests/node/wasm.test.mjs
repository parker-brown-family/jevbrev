// The npm package must print exactly what the Rust crate prints. These compare the WebAssembly
// build with the golden files the Rust tests check. No test here calls Jev.
//   node --test tests/node/wasm.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { Brev } = require("../../npm/jevbrev.js");
const root = new URL("../../", import.meta.url);
const read = (p) => readFileSync(new URL(p, root), "utf8");
const golden = (p) => JSON.parse(read(`tests/golden/${p}`));

const MOVE = "move Alex's Tuesday 10:00s to Thursday at 14:30 from next month";
const SAM = "Sam used to come Thursdays at 5, but move his current Monday 9:30s to Monday 8:30";
const brev = Brev.load(read("examples/clinic.brev"));

test("call 1 matches the crate", () => {
  assert.deepEqual(brev.menu(MOVE), golden("menu.move.json"));
});

test("moving a series, end to end", () => {
  const call2 = brev.frames("move_client_series", MOVE, { client: ["Alex Morgan", "Sam Ortiz", "Jordan Lee"] });
  assert.deepEqual(call2, golden("frames.move.json"));
  const r = brev.read("move_client_series", call2, JSON.parse(read("tests/fixtures/answers.move.json")));
  assert.deepEqual(r, golden("read.move.json"));
  assert.deepEqual(brev.httpCall(r), golden("http.move.json"));
  assert.equal(brev.askFor(r), null);
});

test("Sam: agreeing candidates merge, and the missing input is asked for", () => {
  const call2 = brev.frames("move_client_series", SAM);
  assert.deepEqual(call2, golden("frames.sam.json"));
  const r = brev.read("move_client_series", call2, JSON.parse(read("tests/fixtures/answers.sam.json")));
  assert.deepEqual(r, golden("read.sam.json"));
  assert.deepEqual(brev.askFor(r), golden("ask.sam.json"));
  assert.throws(() => brev.httpCall(r), /not complete/);
});

test("reading call 1", () => {
  const ans = { answers: { action: { type: "choice", choice: "cancel_booking", probabilities: { cancel_booking: 0.45, move_client_series: 0.35, none_of_these: 0.2 } } } };
  assert.deepEqual(brev.readAction(ans), { unsure: true, p: 0.45 });
  assert.deepEqual(brev.readAction({}), { unasked: true });
});

test("load throws every problem with its line; check returns them", () => {
  const bad = read("tests/fixtures/bad.brev");
  assert.throws(() => Brev.load(bad), /1  base URL[\s\S]*13  ref:nothing/);
  const diags = Brev.check(bad);
  assert.equal(diags.length, 8);
  assert.deepEqual(diags[0], { line: 1, message: 'base URL must start with http:// or https://, not "ftp://nowhere"' });
  assert.deepEqual(Brev.check(read("examples/clinic.brev")), []);
});

test("side outputs and import match the crate", () => {
  assert.equal(brev.llm(), read("tests/golden/clinic.llm.txt"));
  assert.deepEqual(brev.tools("anthropic"), golden("clinic.tools.anthropic.json"));
  assert.deepEqual(brev.openapi(), golden("clinic.openapi.json"));
  assert.equal(Brev.importOpenapi(read("tests/fixtures/shop.openapi.json"), "shop.openapi.json"), read("tests/golden/shop.brev"));
  assert.deepEqual(brev.actions(), ["find_client", "list_bookings", "move_client_series", "cancel_booking"]);
});

test("given values skip the question", () => {
  const call2 = brev.frames("cancel_booking", "cancel Jordan's booking on Friday", { booking: "b_42" });
  assert.deepEqual(call2.questions, {});
  const r = brev.read("cancel_booking", call2, {});
  assert.equal(r.complete, true);
});

test("re-import keeps the words", () => {
  const openapi = JSON.stringify({ openapi: "3.1.0", info: { title: "Shop" }, servers: [{ url: "https://shop.example" }],
    paths: { "/items": { get: { operationId: "find", parameters: [
      { name: "name", in: "query", required: true, schema: { type: "string" } },
      { name: "colour", in: "query", schema: { type: "string" } }] } } } });
  const existing = "api Shop  https://shop.example\n\nread find  GET /items\n  Find items by name\n  name  str  which item, by name\n";
  const { text, report } = Brev.mergeOpenapi(openapi, "shop.json", existing);
  assert.match(text, /name +str +which item, by name/);
  assert.match(text, /colour\? +str +\?/);
  assert.deepEqual(report.added, ["find.colour"]);
});
