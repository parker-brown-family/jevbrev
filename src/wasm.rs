//! The JavaScript and TypeScript surface, built with `--features wasm`.

use crate::{frames, model::Brev as Inner, out, parse, read};
use serde::Serialize;
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}

fn from_js(v: JsValue, what: &str) -> Result<Value, JsError> {
    serde_wasm_bindgen::from_value(v).map_err(|e| JsError::new(&format!("{what}: {e}")))
}

fn object(v: JsValue, what: &str) -> Result<Option<Map<String, Value>>, JsError> {
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    match from_js(v, what)? {
        Value::Object(m) => Ok(Some(m)),
        _ => Err(JsError::new(&format!("{what} must be an object"))),
    }
}

/// A loaded, checked `.brev` file.
#[wasm_bindgen]
pub struct Brev(Inner);

#[wasm_bindgen]
impl Brev {
    /// Parse and check. Throws with every problem, one per line, as `LINE  MESSAGE`.
    pub fn load(text: &str) -> Result<Brev, JsError> {
        parse::load(text).map(Brev).map_err(|d| {
            JsError::new(
                &d.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })
    }

    /// Every problem in `text` as `[{ line, message }]`, without throwing. Empty when it loads.
    pub fn check(text: &str) -> Result<JsValue, JsError> {
        let diags = match parse::parse(text) {
            Ok(b) => parse::check(&b),
            Err(d) => d,
        };
        to_js(&diags)
    }

    /// A draft `.brev` from an OpenAPI 3 JSON document.
    #[wasm_bindgen(js_name = importOpenapi)]
    pub fn import_openapi(json: &str, source: &str) -> Result<String, JsError> {
        crate::import::import_openapi(json, source).map_err(|e| JsError::new(&e))
    }

    /// Call 1: the action menu. `context` fields go into `state` beside `request`.
    pub fn menu(&self, request: &str, context: JsValue) -> Result<JsValue, JsError> {
        let ctx = object(context, "context")?;
        to_js(&frames::menu(&self.0, request, ctx.as_ref()))
    }

    /// Read call 1's answers: `{ action, p }`, `{ unsure: true, p }` or `{ unasked: true }`.
    #[wasm_bindgen(js_name = readAction)]
    pub fn read_action(&self, answers: JsValue) -> Result<JsValue, JsError> {
        to_js(&read::read_action(&self.0, &from_js(answers, "answers")?))
    }

    /// Call 2: every question about `action`'s inputs. `known` maps an input to the values
    /// it may take, such as client names.
    pub fn frames(
        &self,
        action: &str,
        request: &str,
        known: JsValue,
        context: JsValue,
    ) -> Result<JsValue, JsError> {
        let known = object(known, "known")?;
        let ctx = object(context, "context")?;
        let v = frames::frames(&self.0, action, request, known.as_ref(), ctx.as_ref())
            .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&v)
    }

    /// Read call 2's answers against the call 2 body they answer.
    pub fn read(&self, action: &str, call2: JsValue, answers: JsValue) -> Result<JsValue, JsError> {
        let r = read::read(
            &self.0,
            action,
            &from_js(call2, "call2")?,
            &from_js(answers, "answers")?,
        )
        .map_err(|e| JsError::new(&e.to_string()))?;
        to_js(&r)
    }

    fn read_of(v: JsValue) -> Result<read::Read, JsError> {
        serde_json::from_value(from_js(v, "read")?).map_err(|e| JsError::new(&format!("read: {e}")))
    }

    /// The HTTP call for a complete read. Throws when the read is not complete.
    #[wasm_bindgen(js_name = httpCall)]
    pub fn http_call(&self, read: JsValue) -> Result<JsValue, JsError> {
        to_js(&out::http_call(&self.0, &Self::read_of(read)?).map_err(|e| JsError::new(&e))?)
    }

    /// What to ask the person next, or `null` when nothing is missing.
    #[wasm_bindgen(js_name = askFor)]
    pub fn ask_for(&self, read: JsValue) -> Result<JsValue, JsError> {
        to_js(&read::ask_for(&self.0, &Self::read_of(read)?))
    }

    /// The short text a language model reads to call the API.
    pub fn llm(&self) -> String {
        out::llm(&self.0)
    }

    /// Tool definitions: `anthropic`, `openai` or `mcp`.
    pub fn tools(&self, style: &str) -> Result<JsValue, JsError> {
        to_js(&out::tools(&self.0, style).map_err(|e| JsError::new(&e))?)
    }

    /// OpenAPI 3.1, with the words kept as `x-jevbrev`.
    pub fn openapi(&self) -> Result<JsValue, JsError> {
        to_js(&out::openapi(&self.0))
    }

    /// The action names, in file order.
    pub fn actions(&self) -> Vec<String> {
        self.0.actions.iter().map(|a| a.name.clone()).collect()
    }
}
