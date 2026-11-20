//! Golden tests: jevbrev's output for fixed inputs, compared with stored files.
//! No test here calls Jev. `JEVBREV_BLESS=1 cargo test --test golden` rewrites the stored files.

use serde_json::{Value, json};
use std::path::PathBuf;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(path(&format!("tests/fixtures/{name}"))).unwrap()
}

fn golden(name: &str, actual: &str) {
    let p = path(&format!("tests/golden/{name}"));
    if std::env::var_os("JEVBREV_BLESS").is_some() {
        std::fs::write(&p, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&p)
        .unwrap_or_else(|_| panic!("missing {}; run with JEVBREV_BLESS=1", p.display()));
    if expected != actual {
        let mut a = p.clone();
        a.set_extension("actual.json");
        std::fs::write(&a, actual).unwrap();
        panic!(
            "{} differs from what jevbrev printed; see {}",
            p.display(),
            a.display()
        );
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap() + "\n"
}

fn clinic() -> jevbrev::Brev {
    jevbrev::load(&std::fs::read_to_string(path("examples/clinic.brev")).unwrap()).unwrap()
}

const PRIYA: &str = "shift Priya's Wednesday 10:00s to Friday at 11:00 from next month";
const SAM: &str =
    "Sam used to come Thursdays at 5, but move his current Monday 9:30s to Monday 8:30";

#[test]
fn menu() {
    golden(
        "menu.priya.json",
        &pretty(&jevbrev::menu(&clinic(), PRIYA, None)),
    );
}

#[test]
fn priya_end_to_end() {
    let b = clinic();
    let known = json!({ "client": ["Priya Shah", "Sam Ortiz", "Jordan Lee"] });
    let call2 = jevbrev::frames(&b, "move_client_series", PRIYA, known.as_object(), None).unwrap();
    golden("frames.priya.json", &pretty(&call2));
    let answers: Value = serde_json::from_str(&fixture("answers.priya.json")).unwrap();
    let read = jevbrev::read(&b, "move_client_series", &call2, &answers).unwrap();
    golden(
        "read.priya.json",
        &pretty(&serde_json::to_value(&read).unwrap()),
    );
    assert!(read.complete);
    golden(
        "http.priya.json",
        &pretty(&jevbrev::http_call(&b, &read).unwrap()),
    );
    assert_eq!(jevbrev::ask_for(&b, &read), None);
}

#[test]
fn sam_merges_agreeing_candidates_and_asks_for_what_is_missing() {
    let b = clinic();
    let call2 = jevbrev::frames(&b, "move_client_series", SAM, None, None).unwrap();
    golden("frames.sam.json", &pretty(&call2));
    let answers: Value = serde_json::from_str(&fixture("answers.sam.json")).unwrap();
    let read = jevbrev::read(&b, "move_client_series", &call2, &answers).unwrap();
    golden(
        "read.sam.json",
        &pretty(&serde_json::to_value(&read).unwrap()),
    );
    // Both "Monday"s are `stays`: one value, not a conflict.
    assert_eq!(read.inputs["from_weekday"]["value"], "Monday");
    assert_eq!(read.inputs["to_weekday"]["value"], "Monday");
    assert_eq!(read.inputs["from_time"]["value"], "9:30");
    assert!(!read.complete);
    // The client was said but there were no known names to read it from.
    let ask = jevbrev::ask_for(&b, &read).unwrap();
    golden("ask.sam.json", &pretty(&ask));
    assert!(jevbrev::http_call(&b, &read).is_err());
}

#[test]
fn a_missing_answer_is_unasked_never_unsaid() {
    let b = clinic();
    let call2 = jevbrev::frames(&b, "move_client_series", SAM, None, None).unwrap();
    let mut answers: Value = serde_json::from_str(&fixture("answers.sam.json")).unwrap();
    answers["answers"].as_object_mut().unwrap().remove("role.3");
    answers["answers"]
        .as_object_mut()
        .unwrap()
        .remove("said.count");
    let read = jevbrev::read(&b, "move_client_series", &call2, &answers).unwrap();
    for name in [
        "from_weekday",
        "from_time",
        "to_weekday",
        "to_time",
        "count",
    ] {
        assert_eq!(read.inputs[name]["state"], "unasked", "{name}");
    }
    assert_eq!(read.inputs["starting"]["state"], "unsaid");
    assert_eq!(
        jevbrev::read_action(&b, &json!({})),
        jevbrev::ActionPick::Unasked { unasked: true }
    );
}

#[test]
fn action_answers() {
    let b = clinic();
    let a = |choice: &str, p: f64| json!({ "answers": { "action": { "type": "choice", "choice": choice, "probabilities": { choice: p, "other": 1.0 - p } } } });
    assert_eq!(
        jevbrev::read_action(&b, &a("move_client_series", 0.98)),
        jevbrev::ActionPick::Action {
            action: "move_client_series".into(),
            p: 0.98
        }
    );
    assert_eq!(
        jevbrev::read_action(&b, &a("none_of_these", 0.9)),
        jevbrev::ActionPick::Action {
            action: "none_of_these".into(),
            p: 0.9
        }
    );
    let split = json!({ "answers": { "action": { "type": "choice", "choice": "cancel_booking",
        "probabilities": { "cancel_booking": 0.45, "move_client_series": 0.35, "none_of_these": 0.2 } } } });
    assert_eq!(
        jevbrev::read_action(&b, &split),
        jevbrev::ActionPick::Unsure {
            unsure: true,
            p: 0.45
        }
    );
    assert!(matches!(
        jevbrev::read_action(&b, &a("not_an_action", 0.9)),
        jevbrev::ActionPick::Unasked { .. }
    ));
}

#[test]
fn a_call2_from_other_spotters_is_refused() {
    let b = clinic();
    let mut call2 = jevbrev::frames(&b, "move_client_series", SAM, None, None).unwrap();
    call2["state"]["candidates"][4] = json!("9");
    assert!(jevbrev::read(&b, "move_client_series", &call2, &json!({})).is_err());
}

#[test]
fn side_outputs() {
    let b = clinic();
    golden("clinic.llm.txt", &jevbrev::llm(&b));
    golden(
        "clinic.tools.anthropic.json",
        &pretty(&jevbrev::tools(&b, "anthropic").unwrap()),
    );
    golden("clinic.openapi.json", &pretty(&jevbrev::openapi(&b)));
    assert!(jevbrev::tools(&b, "nope").is_err());
}

#[test]
fn import_and_check() {
    let draft =
        jevbrev::import_openapi(&fixture("shop.openapi.json"), "shop.openapi.json").unwrap();
    golden("shop.brev", &draft);
    let diags = jevbrev::check(&jevbrev::parse(&draft).unwrap());
    golden(
        "shop.check.txt",
        &diags.iter().map(|d| format!("{d}\n")).collect::<String>(),
    );
    let bad = match jevbrev::parse(&fixture("bad.brev")) {
        Ok(b) => jevbrev::check(&b),
        Err(d) => d,
    };
    golden(
        "bad.check.txt",
        &bad.iter().map(|d| format!("{d}\n")).collect::<String>(),
    );
}

#[test]
fn the_shipped_example_and_spec_example_agree() {
    // docs/using-brev.md quotes the example; this catches the two drifting apart.
    let doc = std::fs::read_to_string(path("docs/using-brev.md")).unwrap();
    let file = std::fs::read_to_string(path("examples/clinic.brev")).unwrap();
    for line in file
        .lines()
        .filter(|l| l.starts_with("  from_weekday") || l.starts_with("  stays="))
    {
        assert!(
            doc.contains(line),
            "docs/using-brev.md does not quote: {line}"
        );
    }
}

#[test]
fn given_values_are_not_asked_and_an_empty_call2_is_read_without_sending() {
    let b = clinic();
    let call2 = jevbrev::frames(
        &b,
        "cancel_booking",
        "cancel Jordan's booking on Friday",
        json!({ "booking": "b_42" }).as_object(),
        None,
    )
    .unwrap();
    assert!(
        call2["questions"].as_object().unwrap().is_empty(),
        "nothing to ask, so nothing to send"
    );
    assert_eq!(call2["state"]["given"]["booking"], "b_42");
    let read = jevbrev::read(&b, "cancel_booking", &call2, &json!({})).unwrap();
    assert!(read.complete);
    assert_eq!(read.inputs["booking"]["value"], "b_42");
}

#[test]
fn one_of_inputs_are_picked_from_their_values() {
    let b = jevbrev::load("api X  https://x.example\n\nchange hours  POST /hours\n  Change the opening hours\n  existing  one:keep|reschedule  whether bookings outside the new hours are kept or rescheduled\n").unwrap();
    let call2 =
        jevbrev::frames(&b, "hours", "close at 4 and keep anyone booked", None, None).unwrap();
    golden("frames.one-of.json", &pretty(&call2));
    let answers = json!({ "answers": {
        "said.existing": { "type": "noul", "noul": 0.95 },
        "pick.existing": { "type": "choice", "choice": "keep", "probabilities": { "keep": 0.93, "reschedule": 0.05, "none_of_these": 0.02 } }
    } });
    let read = jevbrev::read(&b, "hours", &call2, &answers).unwrap();
    assert_eq!(read.inputs["existing"]["value"], "keep");
    let tools = jevbrev::tools(&b, "openai").unwrap();
    assert_eq!(
        tools[0]["function"]["parameters"]["properties"]["existing"]["enum"],
        json!(["keep", "reschedule"])
    );
}

#[test]
fn a_pick_that_disagrees_with_its_noul_is_unsure() {
    let b = clinic();
    let call2 = jevbrev::frames(&b, "move_client_series", PRIYA, None, None).unwrap();
    let mut answers: Value = serde_json::from_str(&fixture("answers.priya.json")).unwrap();
    answers["answers"]["said.starting"] = json!({ "type": "noul", "noul": 0.2 });
    let read = jevbrev::read(&b, "move_client_series", &call2, &answers).unwrap();
    assert_eq!(read.inputs["starting"]["state"], "unsure");
}
