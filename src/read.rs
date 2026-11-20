//! Reading Jev's answers back into an action and the inputs a request said.
//!
//! Absence is kept as its own state everywhere: an input the request did not say is
//! `unsaid`, and an input whose question got no answer is `unasked`. The two are never merged.

use crate::frames::{NONE, action_groups, candidates_key, group_candidates, role_id};
use crate::model::*;
use crate::spot::normalise;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// An answer at or above this probability counts.
pub const THRESHOLD: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// The request says it.
    Said,
    /// The request does not say it.
    Unsaid,
    /// Jev's answer was not clear enough, or two different values claim it.
    Unsure,
    /// No answer came back for its question.
    Unasked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputRead {
    pub state: State,
    /// A span of the request, or a `known` value as passed. Absent when there is none to give,
    /// including when the request says it but the type has nothing to read it from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The probability behind the state. Absent when no single answer decided it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<f64>,
    /// For `unsure` from a conflict: the different values that claimed the input.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Read {
    pub action: String,
    pub inputs: Map<String, Value>,
    /// Every required input is `said` and has a value.
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionPick {
    Unasked { unasked: bool },
    Unsure { unsure: bool, p: f64 },
    Action { action: String, p: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadError(pub String);

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ReadError {}

/// Accept either the whole `/v1/systemone` response or just its `answers` map.
fn answers_of(v: &Value) -> Option<&Map<String, Value>> {
    let obj = v.as_object()?;
    match obj.get("answers") {
        Some(Value::Object(a)) => Some(a),
        _ => Some(obj),
    }
}

/// The top option of a Choice answer and its probability. `None` when the answer is missing
/// or carries no probabilities.
fn top_choice(ans: Option<&Value>) -> Option<(String, f64)> {
    let probs = ans?.get("probabilities")?.as_object()?;
    let mut best: Option<(String, f64)> = None;
    for (k, v) in probs {
        let p = v.as_f64()?;
        if best.as_ref().is_none_or(|(_, bp)| p > *bp) {
            best = Some((k.clone(), p));
        }
    }
    best
}

fn noul(ans: Option<&Value>) -> Option<f64> {
    ans?.get("noul")?.as_f64()
}

/// Read call 1's answer.
pub fn read_action(b: &Brev, answers: &Value) -> ActionPick {
    let Some(a) = answers_of(answers) else {
        return ActionPick::Unasked { unasked: true };
    };
    let Some((choice, p)) = top_choice(a.get("action")) else {
        return ActionPick::Unasked { unasked: true };
    };
    if p < THRESHOLD {
        return ActionPick::Unsure { unsure: true, p };
    }
    if choice != NONE && b.action(&choice).is_none() {
        return ActionPick::Unasked { unasked: true };
    }
    ActionPick::Action { action: choice, p }
}

fn put(out: &mut Map<String, Value>, name: &str, r: InputRead) {
    out.insert(
        name.to_string(),
        serde_json::to_value(r).expect("InputRead serialises"),
    );
}

/// Read call 2's answers for `action`, against the call 2 body they answer.
pub fn read(b: &Brev, action: &str, call2: &Value, answers: &Value) -> Result<Read, ReadError> {
    let a = b
        .action(action)
        .ok_or_else(|| ReadError(format!("no action named {action:?}")))?;
    let state = call2
        .get("state")
        .and_then(Value::as_object)
        .ok_or_else(|| ReadError("call 2 has no state".into()))?;
    let request = state
        .get("request")
        .and_then(Value::as_str)
        .ok_or_else(|| ReadError("call 2's state has no request".into()))?;
    let asked = call2
        .get("questions")
        .and_then(Value::as_object)
        .ok_or_else(|| ReadError("call 2 has no questions".into()))?;
    let empty = Map::new();
    let ans = answers_of(answers).unwrap_or(&empty);
    let mut inputs = Map::new();

    // Inputs in role groups: each candidate's role decides which inputs it fills.
    let groups = action_groups(a);
    for g in &groups {
        let cands = group_candidates(a, g, request);
        let sent: Vec<&str> = state
            .get(&candidates_key(&groups, g))
            .and_then(Value::as_array)
            .map(|v| v.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if sent
            != cands
                .iter()
                .map(|(_, s)| s.text.as_str())
                .collect::<Vec<_>>()
        {
            return Err(ReadError(format!(
                "call 2's candidates for {g} are not what this file's spotters find in its request"
            )));
        }
        let group = b
            .group(g)
            .ok_or_else(|| ReadError(format!("no role group {g}")))?;
        let mut missing = false;
        // input name -> (span text, p) claims
        let mut claims: std::collections::BTreeMap<&str, Vec<(String, f64)>> = Default::default();
        for (n, (ty, span)) in cands.iter().enumerate() {
            let Some((role, p)) = top_choice(ans.get(&role_id(&groups, g, n))) else {
                missing = true;
                continue;
            };
            if p < THRESHOLD {
                continue;
            }
            let mut fills = vec![role.clone()];
            if let Some(r) = group.roles.iter().find(|r| r.name == role) {
                fills.extend(r.covers.iter().cloned());
            }
            for i in a.inputs.iter() {
                if let Some((ig, ir)) = &i.role {
                    if ig == g && fills.contains(ir) && &i.ty == ty {
                        claims
                            .entry(i.name.as_str())
                            .or_default()
                            .push((span.text.clone(), p));
                    }
                }
            }
        }
        for i in a
            .inputs
            .iter()
            .filter(|i| matches!(&i.role, Some((ig, _)) if ig == g))
        {
            let r = if missing {
                InputRead {
                    state: State::Unasked,
                    value: None,
                    p: None,
                    claims: vec![],
                }
            } else {
                let mine = claims.remove(i.name.as_str()).unwrap_or_default();
                let mut distinct: Vec<(String, f64)> = Vec::new();
                for (text, p) in mine {
                    match distinct
                        .iter_mut()
                        .find(|(t, _)| normalise(t) == normalise(&text))
                    {
                        Some(d) => d.1 = d.1.max(p),
                        None => distinct.push((text, p)),
                    }
                }
                match distinct.len() {
                    0 => InputRead {
                        state: State::Unsaid,
                        value: None,
                        p: None,
                        claims: vec![],
                    },
                    1 => {
                        let (text, p) = distinct.remove(0);
                        InputRead {
                            state: State::Said,
                            value: Some(text),
                            p: Some(p),
                            claims: vec![],
                        }
                    }
                    _ => InputRead {
                        state: State::Unsure,
                        value: None,
                        p: None,
                        claims: distinct.into_iter().map(|(t, _)| t).collect(),
                    },
                }
            };
            put(&mut inputs, &i.name, r);
        }
    }

    // Every other input: a Noul for whether it is said, and a pick for its value when there
    // were candidates to pick from.
    let given = state.get("given").and_then(Value::as_object);
    for i in a.inputs.iter().filter(|i| i.role.is_none()) {
        if let Some(Value::String(v)) = given.and_then(|g| g.get(&i.name)) {
            put(
                &mut inputs,
                &i.name,
                InputRead {
                    state: State::Said,
                    value: Some(v.clone()),
                    p: None,
                    claims: vec![],
                },
            );
            continue;
        }
        let said_id = format!("said.{}", i.name);
        let pick_id = format!("pick.{}", i.name);
        if !asked.contains_key(&said_id) {
            put(
                &mut inputs,
                &i.name,
                InputRead {
                    state: State::Unasked,
                    value: None,
                    p: None,
                    claims: vec![],
                },
            );
            continue;
        }
        let Some(np) = noul(ans.get(&said_id)) else {
            put(
                &mut inputs,
                &i.name,
                InputRead {
                    state: State::Unasked,
                    value: None,
                    p: None,
                    claims: vec![],
                },
            );
            continue;
        };
        let said = np >= THRESHOLD;
        let r = if !asked.contains_key(&pick_id) {
            let state = if said { State::Said } else { State::Unsaid };
            InputRead {
                state,
                value: None,
                p: Some(np),
                claims: vec![],
            }
        } else {
            match top_choice(ans.get(&pick_id)) {
                None => InputRead {
                    state: State::Unasked,
                    value: None,
                    p: None,
                    claims: vec![],
                },
                Some((choice, pp)) => match (said, pp >= THRESHOLD, choice == NONE) {
                    (true, true, false) => InputRead {
                        state: State::Said,
                        value: Some(choice),
                        p: Some(np.min(pp)),
                        claims: vec![],
                    },
                    (false, true, true) => InputRead {
                        state: State::Unsaid,
                        value: None,
                        p: Some(np),
                        claims: vec![],
                    },
                    (false, false, _) => InputRead {
                        state: State::Unsaid,
                        value: None,
                        p: Some(np),
                        claims: vec![],
                    },
                    _ => InputRead {
                        state: State::Unsure,
                        value: None,
                        p: None,
                        claims: vec![],
                    },
                },
            }
        };
        put(&mut inputs, &i.name, r);
    }

    // Keep the file's input order.
    let mut ordered = Map::new();
    for i in &a.inputs {
        if let Some(v) = inputs.remove(&i.name) {
            ordered.insert(i.name.clone(), v);
        }
    }
    let complete = a.inputs.iter().filter(|i| !i.optional).all(|i| {
        let r: Option<InputRead> = ordered
            .get(&i.name)
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        matches!(
            r,
            Some(InputRead {
                state: State::Said,
                value: Some(_),
                ..
            })
        )
    });
    Ok(Read {
        action: action.to_string(),
        inputs: ordered,
        complete,
    })
}

/// What to ask the person next: the first required input, in file order, that is `unsaid`,
/// `unsure`, or said without a value. `unasked` inputs are skipped: retry the call for those.
pub fn ask_for(b: &Brev, read: &Read) -> Option<Value> {
    let a = b.action(&read.action)?;
    for i in a.inputs.iter().filter(|i| !i.optional) {
        let r: InputRead = serde_json::from_value(read.inputs.get(&i.name)?.clone()).ok()?;
        let needs = match r.state {
            State::Unsaid | State::Unsure => true,
            State::Said => r.value.is_none(),
            State::Unasked => false,
        };
        if needs {
            let words = i.words.clone().unwrap_or_default();
            return Some(serde_json::json!({
                "input": i.name,
                "state": r.state,
                "text": format!("I need to know {words}."),
            }));
        }
    }
    None
}
