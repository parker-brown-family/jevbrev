//! Re-importing OpenAPI into a `.brev` that already has words, without losing them.
//!
//! What the API decides comes from the new OpenAPI: which actions and inputs exist, each
//! action's method and path, and whether an input is required. What a person decided is kept
//! from the existing file: the words, summaries, confirmed read or change, types, roles,
//! role groups and examples. Anything new arrives unwritten (`?`), so `check` lists it. Anything
//! the API no longer has is listed in comments at the end of the file, words and all, rather
//! than dropped silently.

use crate::fmt::{leading_comments, to_text, trailing_comments};
use crate::import::import_openapi;
use crate::model::*;
use crate::parse::parse;
use serde::Serialize;

/// What a merge did, for the person or agent to read before keeping the file.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MergeReport {
    /// `action` or `action.input`: arrived from the API, words unwritten.
    pub added: Vec<String>,
    /// `action` or `action.input`: no longer in the API; listed at the end of the file.
    pub removed: Vec<String>,
    /// `action: OLD -> NEW` for a method or path the API changed.
    pub moved: Vec<String>,
    /// `action.input`: now required or now optional.
    pub requirement_changed: Vec<String>,
}

fn diags_to_string(d: Vec<Diag>, what: &str) -> String {
    d.iter()
        .map(|x| format!("{what}:{x}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Merge `old` (an existing `.brev`) with `new` (a fresh draft from the API).
pub fn merge(old: &Brev, new: &Brev) -> (Brev, MergeReport, Vec<String>) {
    let mut report = MergeReport::default();
    let mut gone: Vec<String> = Vec::new();
    let mut actions = Vec::new();

    for na in &new.actions {
        let Some(oa) = old.action(&na.name) else {
            report.added.push(na.name.clone());
            actions.push(na.clone());
            continue;
        };
        if (oa.method.as_str(), oa.path.as_str()) != (na.method.as_str(), na.path.as_str()) {
            report.moved.push(format!(
                "{}: {} {} -> {} {}",
                na.name, oa.method, oa.path, na.method, na.path
            ));
        }
        let mut inputs = Vec::new();
        for ni in &na.inputs {
            match oa.input(&ni.name) {
                Some(oi) => {
                    if oi.optional != ni.optional {
                        report.requirement_changed.push(format!(
                            "{}.{}: now {}",
                            na.name,
                            ni.name,
                            if ni.optional { "optional" } else { "required" }
                        ));
                    }
                    let words_written = oi.words.is_some();
                    inputs.push(Input {
                        name: ni.name.clone(),
                        optional: ni.optional,
                        ty: if words_written || oi.ty != Type::Str {
                            oi.ty.clone()
                        } else {
                            ni.ty.clone()
                        },
                        role: oi.role.clone(),
                        words: oi.words.clone().or_else(|| ni.words.clone()),
                        line: 0,
                    });
                }
                None => {
                    report.added.push(format!("{}.{}", na.name, ni.name));
                    inputs.push(ni.clone());
                }
            }
        }
        for oi in &oa.inputs {
            if na.input(&oi.name).is_none() {
                report.removed.push(format!("{}.{}", oa.name, oi.name));
                gone.push(format!(
                    "#   {}.{}  {}  {}",
                    oa.name,
                    oi.name,
                    oi.ty,
                    oi.words.as_deref().unwrap_or("?")
                ));
            }
        }
        let (kind, kind_guessed) = if oa.kind_guessed {
            (na.kind, true)
        } else {
            (oa.kind, false)
        };
        actions.push(Action {
            name: na.name.clone(),
            kind,
            kind_guessed,
            method: na.method.clone(),
            path: na.path.clone(),
            summary: oa.summary.clone().or_else(|| na.summary.clone()),
            inputs,
            yes: oa.yes.clone(),
            no: oa.no.clone(),
            line: 0,
        });
    }
    for oa in &old.actions {
        if new.action(&oa.name).is_none() {
            report.removed.push(oa.name.clone());
            gone.push(format!(
                "#   {} {}  {} {}  {}",
                oa.kind.as_str(),
                oa.name,
                oa.method,
                oa.path,
                oa.summary.as_deref().unwrap_or("?")
            ));
        }
    }

    let api = if old.api.base == "?" {
        new.api.clone()
    } else {
        old.api.clone()
    };
    let merged = Brev {
        api,
        request: old.request.clone(),
        groups: old.groups.clone(),
        actions,
    };
    (merged, report, gone)
}

/// `jevbrev import OPENAPI.json --into EXISTING.brev`: the merged file, and what changed.
pub fn import_into(
    openapi: &str,
    source: &str,
    existing: &str,
    existing_name: &str,
) -> Result<(String, MergeReport), String> {
    let old = parse(existing).map_err(|d| diags_to_string(d, existing_name))?;
    let draft = import_openapi(openapi, source)?;
    let new = parse(&draft).map_err(|d| diags_to_string(d, "the imported draft"))?;
    let (merged, report, gone) = merge(&old, &new);
    let mut text = to_text(&merged, &leading_comments(existing));
    let kept_tail = trailing_comments(existing);
    for l in &kept_tail {
        text.push_str(l);
        text.push('\n');
    }
    if !gone.is_empty() {
        text.push_str(&format!(
            "\n# Removed from the API when {source} was imported. Kept here so their words are not lost;\n# delete these lines once nothing needs them.\n"
        ));
        for g in gone {
            text.push_str(&g);
            text.push('\n');
        }
    }
    Ok((text, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::check;

    const OLD: &str = "# Our shop.\n\napi Shop  https://shop.example\nrequest  what the customer typed\n\nread find  GET /items\n  Find items by part of their name\n  name  str  which item, by name\n  yes find the blue mug\n\nchange reprice  PUT /items/{id}/price\n  Change an item's price\n  id      str   which item\n  amount  int   the new price\n  note?   str   why the price changed\n\nchange retire  POST /items/{id}/retire\n  Stop selling an item\n  id  str  which item to stop selling\n";

    const NEW: &str = r##"{ "openapi": "3.1.0", "info": { "title": "Shop" }, "servers": [{ "url": "https://shop.example" }],
      "paths": {
        "/items": { "get": { "operationId": "find", "parameters": [
          { "name": "name", "in": "query", "required": true, "schema": { "type": "string" } },
          { "name": "colour", "in": "query", "schema": { "type": "string" } } ] } },
        "/v2/items/{id}/price": { "put": { "operationId": "reprice", "parameters": [{ "name": "id", "in": "path", "required": true }],
          "requestBody": { "content": { "application/json": { "schema": { "type": "object", "required": ["amount", "currency"],
            "properties": { "amount": { "type": "number" }, "currency": { "type": "string", "enum": ["CAD", "USD"] } } } } } } } },
        "/items/{id}/stock": { "get": { "operationId": "stock", "summary": "How many of an item are left",
          "parameters": [{ "name": "id", "in": "path", "required": true }] } }
      } }"##;

    #[test]
    fn words_survive_and_changes_are_reported() {
        let (text, r) = import_into(NEW, "shop.json", OLD, "shop.brev").unwrap();
        assert!(text.starts_with("# Our shop.\n"));
        let b = parse(&text).unwrap();
        let find = b.action("find").unwrap();
        assert_eq!(find.kind, Kind::Read);
        assert!(!find.kind_guessed, "a confirmed kind stays confirmed");
        assert_eq!(
            find.input("name").unwrap().words.as_deref(),
            Some("which item, by name")
        );
        assert_eq!(find.yes, ["find the blue mug"]);
        let reprice = b.action("reprice").unwrap();
        assert_eq!(reprice.path, "/v2/items/{id}/price");
        assert_eq!(
            reprice.input("amount").unwrap().words.as_deref(),
            Some("the new price")
        );
        assert_eq!(
            reprice.input("currency").unwrap().ty,
            Type::OneOf(vec!["CAD".into(), "USD".into()])
        );
        assert_eq!(r.added, ["find.colour", "reprice.currency", "stock"]);
        assert_eq!(r.removed, ["reprice.note", "retire"]);
        assert_eq!(
            r.moved,
            ["reprice: PUT /items/{id}/price -> PUT /v2/items/{id}/price"]
        );
        assert!(text.contains("#   reprice.note  str  why the price changed"));
        assert!(text.contains("#   change retire  POST /items/{id}/retire  Stop selling an item"));
        // check lists exactly what is left to write.
        let msgs: Vec<String> = check(&b).into_iter().map(|d| d.message).collect();
        assert!(msgs.iter().any(|m| m.contains("colour has no words")));
        assert!(msgs.iter().any(|m| m.contains("currency has no words")));
        assert!(
            msgs.iter().any(|m| m.contains("read? is a guess")),
            "the new action's kind is a guess"
        );
        assert!(!msgs.iter().any(|m| m.contains("amount")));
    }

    #[test]
    fn merging_twice_changes_nothing() {
        let (once, _) = import_into(NEW, "shop.json", OLD, "shop.brev").unwrap();
        let (twice, r) = import_into(NEW, "shop.json", &once, "shop.brev").unwrap();
        assert_eq!(r, MergeReport::default());
        assert_eq!(
            once, twice,
            "the removed list at the end survives a second import"
        );
    }
}
