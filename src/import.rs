//! A draft `.brev` from an OpenAPI 3 document (JSON).
//!
//! The draft carries what OpenAPI knows: the method, the path, the inputs, their plain types
//! and each summary, whole. It cannot know the words Jev's questions are made from, so an input
//! without a description is written as `?` (unwritten), and so is a missing summary. Whether an
//! action reads or changes is guessed from the method and marked `read?` / `change?`. `check`
//! fails until every guess is confirmed and every `?` is written.

use serde_json::{Map, Value};

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn resolve<'a>(doc: &'a Value, v: &'a Value) -> &'a Value {
    let mut cur = v;
    for _ in 0..8 {
        let Some(r) = cur.get("$ref").and_then(Value::as_str) else {
            return cur;
        };
        let Some(ptr) = r.strip_prefix('#') else {
            return cur;
        };
        match doc.pointer(ptr) {
            Some(t) => cur = t,
            None => return cur,
        }
    }
    cur
}

fn brev_type(schema: &Value) -> (String, Option<&'static str>) {
    if let Some(opts) = schema.get("enum").and_then(Value::as_array) {
        let words: Vec<&str> = opts.iter().filter_map(Value::as_str).collect();
        if words.len() == opts.len()
            && words.len() >= 2
            && words
                .iter()
                .all(|w| !w.is_empty() && !w.contains(['|', ' ']))
        {
            return (format!("one:{}", words.join("|")), None);
        }
    }
    let (t, n) = plain_type(schema);
    (t.to_string(), n)
}

fn plain_type(schema: &Value) -> (&'static str, Option<&'static str>) {
    let ty = schema.get("type").and_then(|t| match t {
        Value::String(s) => Some(s.as_str()),
        Value::Array(a) => a.iter().filter_map(Value::as_str).find(|s| *s != "null"),
        _ => None,
    });
    let format = schema.get("format").and_then(Value::as_str);
    match (ty, format) {
        (Some("integer"), _) => ("int", None),
        (Some("number"), _) => ("int", Some("is a decimal number in OpenAPI")),
        (Some("string"), Some("date" | "date-time")) => ("date", None),
        (Some("string"), _) | (None, _) => ("str", None),
        (Some("boolean"), _) => ("str", Some("is a boolean in OpenAPI")),
        (Some("object"), _) => ("str", Some("is an object in OpenAPI")),
        (Some("array"), _) => ("str", Some("is a list in OpenAPI")),
        (Some(_), _) => ("str", None),
    }
}

fn words_for(name: &str, desc: Option<&str>) -> String {
    match desc.map(one_line) {
        Some(d) if !d.is_empty() && d.to_lowercase() != name.to_lowercase().replace('_', " ") => d,
        _ => "?".into(),
    }
}

struct Row {
    name: String,
    ty: String,
    words: String,
}

fn action_name(op: &Value, method: &str, path: &str) -> String {
    if let Some(id) = op.get("operationId").and_then(Value::as_str) {
        return id.to_string();
    }
    let mut s = method.to_string();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        s.push('_');
        s.extend(part.chars().map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        }));
    }
    s.trim_matches('_').replace("__", "_")
}

/// Write a draft `.brev` from an OpenAPI 3 JSON document. `source` names it in the header.
pub fn import_openapi(text: &str, source: &str) -> Result<String, String> {
    let doc: Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "{source} is not JSON ({e}); convert YAML to JSON first, for example with yq -o=json"
        )
    })?;
    match doc.get("openapi").and_then(Value::as_str) {
        Some(v) if v.starts_with('3') => {}
        _ if doc.get("swagger").is_some() => {
            return Err("Swagger 2.0 is not supported; convert it to OpenAPI 3 first".into());
        }
        _ => return Err(format!("{source} has no openapi: \"3.x\" field")),
    }
    let title = doc
        .pointer("/info/title")
        .and_then(Value::as_str)
        .map(one_line)
        .unwrap_or_else(|| "?".into());
    let base = doc
        .pointer("/servers/0/url")
        .and_then(Value::as_str)
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .unwrap_or("?");

    let mut out = format!(
        "# Draft from {source} by jevbrev import.\n# Every ? is unwritten and every read? or change? is a guess. jevbrev check lists what is left.\n\napi {title}  {base}\n"
    );
    let empty = Map::new();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for (path, item) in paths {
        let item = resolve(&doc, item);
        let shared: Vec<Value> = item
            .get("parameters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for method in ["get", "post", "put", "patch", "delete"] {
            let Some(op) = item.get(method) else { continue };
            let upper = method.to_uppercase();
            let kind = if method == "get" { "read?" } else { "change?" };
            let summary = op
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| {
                    op.get("description")
                        .and_then(Value::as_str)
                        .and_then(|d| d.lines().next())
                })
                .map(one_line)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "?".into());
            let mut rows: Vec<Row> = Vec::new();
            let mut notes: Vec<String> = Vec::new();
            let params: Vec<Value> = shared
                .iter()
                .chain(
                    op.get("parameters")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .cloned()
                .collect();
            for p in &params {
                let p = resolve(&doc, p);
                let Some(name) = p.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let place = p.get("in").and_then(Value::as_str).unwrap_or("");
                if place != "path" && place != "query" {
                    notes.push(format!("  # skipped {place} parameter {name}"));
                    continue;
                }
                if rows
                    .iter()
                    .any(|r| r.name == name || r.name == format!("{name}?"))
                {
                    continue;
                }
                let required =
                    place == "path" || p.get("required").and_then(Value::as_bool).unwrap_or(false);
                let schema = p
                    .get("schema")
                    .map(|s| resolve(&doc, s))
                    .cloned()
                    .unwrap_or(Value::Null);
                let (ty, note) = brev_type(&schema);
                if let Some(n) = note {
                    notes.push(format!("  # {name} {n}; jevbrev reads it as {ty}"));
                }
                rows.push(Row {
                    name: if required {
                        name.to_string()
                    } else {
                        format!("{name}?")
                    },
                    ty: ty.clone(),
                    words: words_for(name, p.get("description").and_then(Value::as_str)),
                });
            }
            if let Some(body) = op.get("requestBody").map(|b| resolve(&doc, b)) {
                let schema = body
                    .pointer("/content/application~1json/schema")
                    .map(|s| resolve(&doc, s));
                match schema.and_then(|s| s.get("properties")).and_then(Value::as_object) {
                    Some(props) => {
                        let req: Vec<&str> = schema
                            .and_then(|s| s.get("required"))
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(Value::as_str).collect())
                            .unwrap_or_default();
                        for (name, prop) in props {
                            let prop = resolve(&doc, prop);
                            let (ty, note) = brev_type(prop);
                            if let Some(n) = note {
                                notes.push(format!("  # {name} {n}; jevbrev reads it as {ty}"));
                            }
                            rows.push(Row {
                                name: if req.contains(&name.as_str()) { name.clone() } else { format!("{name}?") },
                                ty: ty.clone(),
                                words: words_for(name, prop.get("description").and_then(Value::as_str)),
                            });
                        }
                    }
                    None if body.get("content").is_some() => {
                        notes.push("  # the request body is not a JSON object with properties; add its inputs by hand".into())
                    }
                    None => {}
                }
            }
            out.push_str(&format!(
                "\n{kind} {}  {upper} {path}\n  {summary}\n",
                action_name(op, method, path)
            ));
            let w0 = rows.iter().map(|r| r.name.len()).max().unwrap_or(0).max(4) + 2;
            let w1 = rows.iter().map(|r| r.ty.len()).max().unwrap_or(0).max(3) + 2;
            for r in &rows {
                out.push_str(&format!("  {:w0$}{:w1$}{}\n", r.name, r.ty, r.words));
            }
            for n in notes {
                out.push_str(&n);
                out.push('\n');
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{check, parse};

    const DOC: &str = r##"{
      "openapi": "3.1.0",
      "info": { "title": "Shop" },
      "servers": [{ "url": "https://shop.example" }],
      "paths": {
        "/items/{id}": {
          "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" }, "description": "which item" }],
          "get": { "operationId": "get_item", "summary": "Show one item, with its price and stock, and every review left on it by a customer this year" },
          "post": {
            "operationId": "reprice",
            "requestBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Price" } } } }
          }
        }
      },
      "components": { "schemas": { "Price": { "type": "object", "required": ["amount"], "properties": {
        "amount": { "type": "number" }, "note": { "type": "string", "description": "why the price changed" } } } } }
    }"##;

    #[test]
    fn draft_parses_and_check_lists_what_is_left() {
        let draft = import_openapi(DOC, "shop.json").unwrap();
        let b = parse(&draft).unwrap();
        assert_eq!(b.actions.len(), 2);
        // The summary is carried whole, never cut.
        assert!(
            b.actions[0]
                .summary
                .as_deref()
                .unwrap()
                .ends_with("by a customer this year")
        );
        let reprice = b.action("reprice").unwrap();
        assert_eq!(reprice.summary, None);
        assert_eq!(reprice.input("amount").unwrap().words, None);
        assert!(reprice.input("note").unwrap().optional);
        let d = check(&b);
        let msgs: Vec<&str> = d.iter().map(|x| x.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("read? is a guess from GET")));
        assert!(
            msgs.iter()
                .any(|m| m.contains("change? is a guess from POST"))
        );
        assert!(msgs.iter().any(|m| m.contains("reprice has no summary")));
        assert!(msgs.iter().any(|m| m.contains("amount has no words")));
        assert!(!msgs.iter().any(|m| m.contains("id has no words")));
    }

    #[test]
    fn yaml_and_swagger_are_refused_with_a_way_forward() {
        assert!(
            import_openapi("openapi: 3.1.0", "x.yaml")
                .unwrap_err()
                .contains("yq")
        );
        assert!(
            import_openapi(r#"{"swagger":"2.0"}"#, "x.json")
                .unwrap_err()
                .contains("Swagger 2.0")
        );
    }
}
