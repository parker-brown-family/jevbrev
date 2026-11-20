//! What jevbrev prints besides Jev requests: the HTTP call for a complete read, a short text
//! for a language model, tool definitions, and OpenAPI.

use crate::model::*;
use crate::read::{InputRead, Read, State};
use serde_json::{Map, Value, json};

/// The HTTP call for a complete read. Values are the request's spans or the `known` values as
/// passed: the caller normalises them, resolves each `ref` to its ID and fills the path.
pub fn http_call(b: &Brev, read: &Read) -> Result<Value, String> {
    let a = b
        .action(&read.action)
        .ok_or_else(|| format!("no action named {:?}", read.action))?;
    if !read.complete {
        let missing: Vec<&str> = a
            .inputs
            .iter()
            .filter(|i| !i.optional)
            .filter(|i| {
                !matches!(
                    read.inputs
                        .get(&i.name)
                        .and_then(|v| serde_json::from_value::<InputRead>(v.clone()).ok()),
                    Some(InputRead {
                        state: State::Said,
                        value: Some(_),
                        ..
                    })
                )
            })
            .map(|i| i.name.as_str())
            .collect();
        return Err(format!(
            "{} is not complete: {} not said with a value",
            a.name,
            missing.join(", ")
        ));
    }
    let path_params = a.path_params();
    let mut path_values = Map::new();
    let mut rest = Map::new();
    for i in &a.inputs {
        let Some(r) = read
            .inputs
            .get(&i.name)
            .and_then(|v| serde_json::from_value::<InputRead>(v.clone()).ok())
        else {
            continue;
        };
        if let (State::Said, Some(v)) = (r.state, r.value) {
            if path_params.contains(&i.name) {
                path_values.insert(i.name.clone(), Value::String(v));
            } else {
                rest.insert(i.name.clone(), Value::String(v));
            }
        }
    }
    let mut out = json!({
        "method": a.method,
        "base_url": b.api.base,
        "path": a.path,
        "path_values": path_values,
        "change": a.kind == Kind::Change,
    });
    let key = if matches!(a.method.as_str(), "GET" | "DELETE" | "HEAD" | "OPTIONS") {
        "query"
    } else {
        "body"
    };
    out[key] = Value::Object(rest);
    Ok(out)
}

fn type_words(ty: &Type) -> String {
    match ty {
        Type::Str => "text".into(),
        Type::Int => "whole number".into(),
        Type::Date => "date".into(),
        Type::Weekday => "weekday".into(),
        Type::Time => "time of day".into(),
        Type::Ref(a) => format!("id of a {a} result"),
        Type::OneOf(o) => format!("one of {}", o.join(", ")),
    }
}

/// A short text a language model reads to call the API.
pub fn llm(b: &Brev) -> String {
    let mut s = format!("{} · {}", b.api.name, b.api.base);
    if b.actions.iter().any(|a| a.kind == Kind::Change) {
        s.push_str(" · change actions return a preview; confirm with the person before running");
    }
    s.push('\n');
    for a in &b.actions {
        s.push_str(&format!(
            "\n{}  {} {}  {}\n",
            a.name,
            a.method,
            a.path,
            a.kind.as_str()
        ));
        s.push_str(&format!("  {}\n", a.summary.as_deref().unwrap_or("")));
        for i in &a.inputs {
            let opt = if i.optional { " (optional)" } else { "" };
            s.push_str(&format!(
                "  {}{opt}: {} — {}\n",
                i.name,
                type_words(&i.ty),
                i.words.as_deref().unwrap_or("")
            ));
        }
    }
    s
}

fn schema(ty: &Type, words: &str) -> Value {
    match ty {
        Type::Int => json!({ "type": "integer", "description": words }),
        Type::Date => json!({ "type": "string", "format": "date", "description": words }),
        Type::Weekday => json!({
            "type": "string",
            "enum": ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
            "description": words,
        }),
        Type::Time => {
            json!({ "type": "string", "pattern": "^[0-2][0-9]:[0-5][0-9]$", "description": format!("{words} (24-hour HH:MM)") })
        }
        Type::Ref(a) => {
            json!({ "type": "string", "description": format!("{words} (an id from {a})") })
        }
        Type::Str => json!({ "type": "string", "description": words }),
        Type::OneOf(o) => json!({ "type": "string", "enum": o, "description": words }),
    }
}

fn input_schema(a: &Action) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for i in &a.inputs {
        props.insert(
            i.name.clone(),
            schema(&i.ty, i.words.as_deref().unwrap_or("")),
        );
        if !i.optional {
            required.push(Value::String(i.name.clone()));
        }
    }
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

/// The actions as tool definitions. `style` is `anthropic` (`input_schema`), `openai`
/// (`type: function` with `parameters`) or `mcp` (`inputSchema`).
pub fn tools(b: &Brev, style: &str) -> Result<Value, String> {
    let mut out = Vec::new();
    for a in &b.actions {
        let mut desc = a.summary.clone().unwrap_or_default();
        if a.kind == Kind::Change {
            desc.push_str(". Returns a preview; nothing changes until the person confirms.");
        }
        let schema = input_schema(a);
        out.push(match style {
            "anthropic" => json!({ "name": a.name, "description": desc, "input_schema": schema }),
            "openai" => json!({ "type": "function", "function": { "name": a.name, "description": desc, "parameters": schema } }),
            "mcp" => json!({ "name": a.name, "description": desc, "inputSchema": schema }),
            other => return Err(format!("unknown tool style {other:?}: use anthropic, openai or mcp")),
        });
    }
    Ok(Value::Array(out))
}

/// OpenAPI 3.1 for documentation tools. The words, types and roles ride along as `x-jevbrev`
/// so nothing is lost.
pub fn openapi(b: &Brev) -> Value {
    let mut paths = Map::new();
    for a in &b.actions {
        let path_params = a.path_params();
        let query = matches!(a.method.as_str(), "GET" | "DELETE" | "HEAD" | "OPTIONS");
        let mut params = Vec::new();
        let mut body_props = Map::new();
        let mut body_required = Vec::new();
        for i in &a.inputs {
            let words = i.words.as_deref().unwrap_or("");
            let mut ext = json!({ "type": i.ty.to_string(), "words": words });
            if let Some((g, r)) = &i.role {
                ext["role"] = Value::String(format!("{g}.{r}"));
            }
            let mut sch = schema(&i.ty, words);
            if let Some(o) = sch.as_object_mut() {
                o.remove("description");
            }
            if path_params.contains(&i.name) || query {
                params.push(json!({
                    "name": i.name,
                    "in": if path_params.contains(&i.name) { "path" } else { "query" },
                    "required": path_params.contains(&i.name) || !i.optional,
                    "description": words,
                    "schema": sch,
                    "x-jevbrev": ext,
                }));
            } else {
                let mut sch = sch;
                sch["description"] = Value::String(words.to_string());
                sch["x-jevbrev"] = ext;
                body_props.insert(i.name.clone(), sch);
                if !i.optional {
                    body_required.push(Value::String(i.name.clone()));
                }
            }
        }
        let mut op = json!({
            "operationId": a.name,
            "summary": a.summary.clone().unwrap_or_default(),
            "x-jevbrev": { "kind": a.kind.as_str(), "yes": a.yes, "no": a.no },
            "responses": { "200": { "description": if a.kind == Kind::Change { "a preview of the change; nothing changes until it is confirmed" } else { "ok" } } },
        });
        if !params.is_empty() {
            op["parameters"] = Value::Array(params);
        }
        if !body_props.is_empty() {
            op["requestBody"] = json!({
                "required": true,
                "content": { "application/json": { "schema": {
                    "type": "object", "properties": body_props, "required": body_required,
                } } },
            });
        }
        let entry = paths.entry(a.path.clone()).or_insert_with(|| json!({}));
        entry[a.method.to_lowercase()] = op;
    }
    let mut roles = Map::new();
    for g in &b.groups {
        let mut m = Map::new();
        for r in &g.roles {
            m.insert(
                r.name.clone(),
                json!({ "covers": r.covers, "words": r.words }),
            );
        }
        roles.insert(g.name.clone(), Value::Object(m));
    }
    json!({
        "openapi": "3.1.0",
        "info": { "title": b.api.name, "version": "0.0.0", "x-jevbrev-request": b.request },
        "servers": [{ "url": b.api.base }],
        "x-jevbrev-roles": roles,
        "paths": paths,
    })
}
