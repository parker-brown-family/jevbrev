//! The `jevbrev` command.

use serde_json::{Map, Value};
use std::io::Read as _;
use std::process::ExitCode;

const USAGE: &str = "\
jevbrev: short API definitions that code turns into Jev questions and HTTP calls

USAGE
  jevbrev check FILE                                   every problem, with its line; exit 1 if any
  jevbrev import OPENAPI.json [--into FILE]            a draft .brev on stdout; with --into, merged into
                                                       FILE keeping its words, and a report on stderr
  jevbrev fmt FILE                                     FILE in the standard layout, on stdout
  jevbrev menu FILE REQUEST [--context JSON]           call 1: which action
  jevbrev action FILE ANSWERS.json                     read call 1's answers
  jevbrev frames FILE ACTION REQUEST [--known JSON] [--context JSON]
                                                       call 2: every input question
  jevbrev read FILE ACTION CALL2.json ANSWERS.json     read call 2's answers
  jevbrev http FILE READ.json                          the HTTP call; exit 1 if the read is not complete
  jevbrev ask FILE READ.json                           what to ask the person next, or null
  jevbrev llm FILE                                     text for a language model
  jevbrev tools FILE [--style anthropic|openai|mcp]    tool definitions
  jevbrev openapi FILE                                 OpenAPI 3.1

A JSON file argument may be - for stdin. jevbrev never calls Jev and never reads a key.
Spec: https://github.com/parker-brown-family/jevbrev/blob/main/docs/using-brev.md
";

enum Fail {
    Usage(String),
    Problem(String),
}

type Res = Result<(), Fail>;

fn usage(m: impl Into<String>) -> Fail {
    Fail::Usage(m.into())
}

fn read_text(path: &str) -> Result<String, Fail> {
    if path == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| Fail::Problem(format!("stdin: {e}")))?;
        return Ok(s);
    }
    std::fs::read_to_string(path).map_err(|e| Fail::Problem(format!("{path}: {e}")))
}

fn read_json(path: &str) -> Result<Value, Fail> {
    serde_json::from_str(&read_text(path)?)
        .map_err(|e| Fail::Problem(format!("{path}: not JSON: {e}")))
}

fn json_object(text: &str, what: &str) -> Result<Map<String, Value>, Fail> {
    match serde_json::from_str(text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(usage(format!("{what} must be a JSON object"))),
        Err(e) => Err(usage(format!("{what}: not JSON: {e}"))),
    }
}

fn load(path: &str) -> Result<jevbrev::Brev, Fail> {
    let text = read_text(path)?;
    jevbrev::load(&text).map_err(|d| {
        Fail::Problem(
            d.iter()
                .map(|x| format!("{path}:{x}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })
}

fn print(v: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(v).expect("a Value always serialises")
    );
}

/// Positional arguments, and `--flag VALUE` pairs.
fn split(args: &[String]) -> Result<(Vec<&str>, Map<String, Value>), Fail> {
    let mut pos = Vec::new();
    let mut flags = Map::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = a.strip_prefix("--") {
            let v = args
                .get(i + 1)
                .ok_or_else(|| usage(format!("--{name} needs a value")))?;
            flags.insert(name.to_string(), Value::String(v.clone()));
            i += 2;
        } else {
            pos.push(a.as_str());
            i += 1;
        }
    }
    Ok((pos, flags))
}

fn flag<'a>(
    flags: &'a Map<String, Value>,
    name: &str,
    allowed: &[&str],
) -> Result<Option<&'a str>, Fail> {
    for k in flags.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(usage(format!("unknown flag --{k}")));
        }
    }
    Ok(flags.get(name).and_then(Value::as_str))
}

fn want<'a>(pos: &[&'a str], n: usize, form: &str) -> Result<Vec<&'a str>, Fail> {
    if pos.len() != n {
        return Err(usage(format!("usage: jevbrev {form}")));
    }
    Ok(pos.to_vec())
}

fn run(args: &[String]) -> Res {
    let Some(cmd) = args.first() else {
        return Err(usage(USAGE));
    };
    let (pos, flags) = split(&args[1..])?;
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("jevbrev {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "check" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 1, "check FILE")?;
            let text = read_text(p[0])?;
            let diags = match jevbrev::parse(&text) {
                Ok(b) => jevbrev::check(&b),
                Err(d) => d,
            };
            if diags.is_empty() {
                println!("{}: ok", p[0]);
                Ok(())
            } else {
                for d in &diags {
                    println!("{}:{d}", p[0]);
                }
                Err(Fail::Problem(format!(
                    "{} problem{}",
                    diags.len(),
                    if diags.len() == 1 { "" } else { "s" }
                )))
            }
        }
        "import" => {
            let into = flag(&flags, "into", &["into"])?;
            let p = want(&pos, 1, "import OPENAPI.json [--into FILE]")?;
            let openapi = read_text(p[0])?;
            match into {
                None => print!(
                    "{}",
                    jevbrev::import_openapi(&openapi, p[0]).map_err(Fail::Problem)?
                ),
                Some(file) => {
                    let (text, report) =
                        jevbrev::import_into(&openapi, p[0], &read_text(file)?, file)
                            .map_err(Fail::Problem)?;
                    print!("{text}");
                    eprintln!(
                        "{}",
                        serde_json::to_string_pretty(&report).expect("serialises")
                    );
                }
            }
            Ok(())
        }
        "fmt" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 1, "fmt FILE")?;
            let text = read_text(p[0])?;
            let b = jevbrev::parse(&text).map_err(|d| {
                Fail::Problem(
                    d.iter()
                        .map(|x| format!("{}:{x}", p[0]))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })?;
            let mut out = jevbrev::to_text(&b, &jevbrev::fmt::leading_comments(&text));
            for l in jevbrev::fmt::trailing_comments(&text) {
                out.push_str(&l);
                out.push('\n');
            }
            print!("{out}");
            Ok(())
        }
        "menu" => {
            let ctx = flag(&flags, "context", &["context"])?
                .map(|c| json_object(c, "--context"))
                .transpose()?;
            let p = want(&pos, 2, "menu FILE REQUEST [--context JSON]")?;
            print(&jevbrev::menu(&load(p[0])?, p[1], ctx.as_ref()));
            Ok(())
        }
        "action" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 2, "action FILE ANSWERS.json")?;
            let pick = jevbrev::read_action(&load(p[0])?, &read_json(p[1])?);
            print(&serde_json::to_value(pick).expect("serialises"));
            Ok(())
        }
        "frames" => {
            flag(&flags, "", &["known", "context"])?;
            let known = flags
                .get("known")
                .and_then(Value::as_str)
                .map(|k| json_object(k, "--known"))
                .transpose()?;
            let ctx = flags
                .get("context")
                .and_then(Value::as_str)
                .map(|c| json_object(c, "--context"))
                .transpose()?;
            let p = want(
                &pos,
                3,
                "frames FILE ACTION REQUEST [--known JSON] [--context JSON]",
            )?;
            let v = jevbrev::frames(&load(p[0])?, p[1], p[2], known.as_ref(), ctx.as_ref())
                .map_err(|e| Fail::Problem(e.to_string()))?;
            print(&v);
            Ok(())
        }
        "read" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 4, "read FILE ACTION CALL2.json ANSWERS.json")?;
            let r = jevbrev::read(&load(p[0])?, p[1], &read_json(p[2])?, &read_json(p[3])?)
                .map_err(|e| Fail::Problem(e.to_string()))?;
            print(&serde_json::to_value(r).expect("serialises"));
            Ok(())
        }
        "http" | "ask" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 2, &format!("{cmd} FILE READ.json"))?;
            let b = load(p[0])?;
            let r: jevbrev::Read = serde_json::from_value(read_json(p[1])?)
                .map_err(|e| Fail::Problem(format!("{}: not a read result: {e}", p[1])))?;
            if cmd == "http" {
                print(&jevbrev::http_call(&b, &r).map_err(Fail::Problem)?);
            } else {
                print(&jevbrev::ask_for(&b, &r).unwrap_or(Value::Null));
            }
            Ok(())
        }
        "llm" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 1, "llm FILE")?;
            print!("{}", jevbrev::llm(&load(p[0])?));
            Ok(())
        }
        "tools" => {
            let style = flag(&flags, "style", &["style"])?.unwrap_or("anthropic");
            let p = want(&pos, 1, "tools FILE [--style anthropic|openai|mcp]")?;
            print(&jevbrev::tools(&load(p[0])?, style).map_err(usage)?);
            Ok(())
        }
        "openapi" => {
            flag(&flags, "", &[])?;
            let p = want(&pos, 1, "openapi FILE")?;
            print(&jevbrev::openapi(&load(p[0])?));
            Ok(())
        }
        other => Err(usage(format!("unknown command {other:?}\n\n{USAGE}"))),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail::Problem(m)) => {
            eprintln!("{m}");
            ExitCode::from(1)
        }
        Err(Fail::Usage(m)) => {
            eprintln!("{m}");
            ExitCode::from(2)
        }
    }
}
