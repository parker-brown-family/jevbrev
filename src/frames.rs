//! The two Jev requests for one person's request: which action (call 1), and every question
//! about that action's inputs (call 2). Both are complete `/v1/systemone` bodies.

use crate::model::*;
use crate::spot::{Span, spot};
use serde_json::{Map, Value, json};

pub const MODEL: &str = "jev-latest";
pub const NONE: &str = "none_of_these";
const NONE_ACTION: &str = "No action here does what `request` asks.";
const NONE_PICK: &str = "`request` does not say this.";

/// What the caller already knows about an input. A list of strings is the values it may take,
/// such as the clinic's client names for a `ref:find_client` input: Jev picks one, and jevbrev
/// never makes one up. A single string is the value itself, such as the client on screen: no
/// question is asked, and it is recorded in `state.given`.
pub type Known = Map<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum FrameError {
    UnknownAction(String),
    BadKnown(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::UnknownAction(a) => write!(f, "no action named {a:?}"),
            FrameError::BadKnown(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for FrameError {}

fn state(request: &str, context: Option<&Map<String, Value>>) -> Map<String, Value> {
    let mut s = Map::new();
    if let Some(c) = context {
        for (k, v) in c {
            s.insert(k.clone(), v.clone());
        }
    }
    s.insert("request".into(), Value::String(request.to_string()));
    s
}

/// Call 1: one Choice whose options are the actions' summaries, word for word, plus
/// `none_of_these`. `context` fields (who is signed in, what is on screen) go into `state`
/// beside `request`.
pub fn menu(b: &Brev, request: &str, context: Option<&Map<String, Value>>) -> Value {
    let mut criteria = Map::new();
    for a in &b.actions {
        criteria.insert(
            a.name.clone(),
            Value::String(a.summary.clone().unwrap_or_default()),
        );
    }
    criteria.insert(NONE.into(), Value::String(NONE_ACTION.into()));
    json!({
        "model": MODEL,
        "state": state(request, context),
        "questions": {
            "action": {
                "type": "choice",
                "instructions": "Which action would carry out `request`? Choose none_of_these when no action does what `request` asks.",
                "criteria": criteria,
            }
        }
    })
}

/// The state key holding a role group's candidates: `candidates` when the action uses one group.
pub(crate) fn candidates_key(groups: &[String], group: &str) -> String {
    if groups.len() == 1 {
        "candidates".into()
    } else {
        format!("candidates_{group}")
    }
}

/// The question id for a role question: `role.N`, or `role.GROUP.N` when the action uses
/// more than one group.
pub(crate) fn role_id(groups: &[String], group: &str, n: usize) -> String {
    if groups.len() == 1 {
        format!("role.{n}")
    } else {
        format!("role.{group}.{n}")
    }
}

/// The role groups an action uses, in the order its inputs first name them.
pub(crate) fn action_groups(a: &Action) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in &a.inputs {
        if let Some((g, _)) = &i.role {
            if !out.contains(g) {
                out.push(g.clone());
            }
        }
    }
    out
}

/// The candidates for a role group: spans for each type the group's inputs use, the types in
/// the order the inputs first use them, each type's spans in the order they appear.
pub(crate) fn group_candidates(a: &Action, group: &str, request: &str) -> Vec<(Type, Span)> {
    let mut types: Vec<Type> = Vec::new();
    for i in &a.inputs {
        if matches!(&i.role, Some((g, _)) if g == group) && !types.contains(&i.ty) {
            types.push(i.ty.clone());
        }
    }
    types
        .into_iter()
        .flat_map(|t| spot(&t, request).into_iter().map(move |s| (t.clone(), s)))
        .collect()
}

fn role_question(b: &Brev, group: &str, key: &str, n: usize, ty: &Type, span: &Span) -> Value {
    let noun = ty.noun().unwrap_or("value");
    let mut criteria = Map::new();
    if let Some(g) = b.group(group) {
        for r in &g.roles {
            criteria.insert(
                r.name.clone(),
                Value::String(r.words.replace("{kind}", noun)),
            );
        }
    }
    json!({
        "type": "choice",
        "instructions": format!("In `request`, what is \"{}\" (`{key}[{n}]`)?", span.text),
        "criteria": criteria,
    })
}

/// Call 2: every question about one action's inputs, in one request.
///
/// - an input in a role group gets no question of its own: each candidate span gets a Choice
///   over the group's roles (`role.N`);
/// - any other input with written words gets a Noul, "Does `request` say WORDS?" (`said.NAME`);
/// - and, when it has candidates (a spotter found spans, or the caller passed `known` values),
///   a Choice over them plus `none_of_these` (`pick.NAME`).
pub fn frames(
    b: &Brev,
    action: &str,
    request: &str,
    known: Option<&Known>,
    context: Option<&Map<String, Value>>,
) -> Result<Value, FrameError> {
    let a = b
        .action(action)
        .ok_or_else(|| FrameError::UnknownAction(action.to_string()))?;
    let mut st = state(request, context);
    let mut questions = Map::new();

    let groups = action_groups(a);
    for g in &groups {
        let cands = group_candidates(a, g, request);
        let key = candidates_key(&groups, g);
        st.insert(
            key.clone(),
            Value::Array(
                cands
                    .iter()
                    .map(|(_, s)| Value::String(s.text.clone()))
                    .collect(),
            ),
        );
        for (n, (ty, span)) in cands.iter().enumerate() {
            questions.insert(
                role_id(&groups, g, n),
                role_question(b, g, &key, n, ty, span),
            );
        }
    }

    if let Some(k) = known {
        for name in k.keys() {
            if a.input(name).is_none() {
                return Err(FrameError::BadKnown(format!(
                    "known names {name:?}, which is not an input of {action}"
                )));
            }
        }
    }

    let mut given = Map::new();
    for i in a.inputs.iter().filter(|i| i.role.is_none()) {
        let Some(words) = &i.words else { continue };
        if let Some(Value::String(v)) = known.and_then(|k| k.get(&i.name)) {
            given.insert(i.name.clone(), Value::String(v.clone()));
            continue;
        }
        questions.insert(
            format!("said.{}", i.name),
            json!({ "type": "noul", "instructions": format!("Does `request` say {words}?") }),
        );
        let cands: Vec<String> = match known.and_then(|k| k.get(&i.name)) {
            // A single value is given: the caller already knows it, so nothing is asked.
            Some(Value::String(_)) => continue,
            Some(Value::Array(vs)) => {
                let mut out = Vec::new();
                for v in vs {
                    match v {
                        Value::String(s) => out.push(s.clone()),
                        other => {
                            return Err(FrameError::BadKnown(format!(
                                "known values for {} must be strings, not {other}",
                                i.name
                            )));
                        }
                    }
                }
                out
            }
            Some(other) => {
                return Err(FrameError::BadKnown(format!(
                    "known for {} must be a string (given) or a list of strings (candidates), not {other}",
                    i.name
                )));
            }
            None => match &i.ty {
                Type::OneOf(opts) => opts.clone(),
                ty => spot(ty, request).into_iter().map(|s| s.text).collect(),
            },
        };
        let mut uniq: Vec<String> = Vec::new();
        for c in cands {
            if !uniq.contains(&c) && c != NONE {
                uniq.push(c);
            }
        }
        if uniq.is_empty() {
            continue;
        }
        if uniq.len() > 254 {
            return Err(FrameError::BadKnown(format!(
                "{} has {} candidates; a Choice takes at most 254 plus none_of_these",
                i.name,
                uniq.len()
            )));
        }
        let key = format!("{}_candidates", i.name);
        st.insert(
            key.clone(),
            Value::Array(uniq.iter().cloned().map(Value::String).collect()),
        );
        let mut criteria = Map::new();
        for c in &uniq {
            criteria.insert(c.clone(), Value::Null);
        }
        criteria.insert(NONE.into(), Value::String(NONE_PICK.into()));
        questions.insert(
            format!("pick.{}", i.name),
            json!({
                "type": "choice",
                "instructions": format!("Which of `{key}` is {words}?"),
                "criteria": criteria,
            }),
        );
    }

    if !given.is_empty() {
        st.insert("given".into(), Value::Object(given));
    }
    Ok(json!({ "model": MODEL, "state": st, "questions": questions }))
}
