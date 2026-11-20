//! Reading a `.brev` file, and checking it.
//!
//! `parse` reads what is there, including unwritten (`?`) words and guessed kinds, so that a
//! draft from `import` can be loaded and reported on. `check` then lists every problem that
//! stops the file from being used. `load` is both, and refuses a file with any problem.

use crate::model::*;

/// Parse and check. Every problem is returned, each with its line.
pub fn load(text: &str) -> Result<Brev, Vec<Diag>> {
    let brev = parse(text)?;
    let diags = check(&brev);
    if diags.is_empty() {
        Ok(brev)
    } else {
        Err(diags)
    }
}

fn diag(line: usize, message: impl Into<String>) -> Diag {
    Diag {
        line,
        message: message.into(),
    }
}

/// Split off `n` whitespace-separated tokens and return them with the rest of the line, trimmed.
fn take(line: &str, n: usize) -> (Vec<&str>, &str) {
    let mut toks = Vec::with_capacity(n);
    let mut rest = line.trim_start();
    for _ in 0..n {
        if rest.is_empty() {
            break;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        toks.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    (toks, rest.trim_end())
}

pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

const RESERVED: [&str; 3] = ["yes", "no", "none_of_these"];
const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

enum Block {
    None,
    Roles(usize),
    Action(usize),
}

/// Read the file. Syntax errors are returned; problems of meaning are left to `check`.
pub fn parse(text: &str) -> Result<Brev, Vec<Diag>> {
    let mut diags = Vec::new();
    let mut api: Option<Api> = None;
    let mut request = None;
    let mut groups: Vec<RoleGroup> = Vec::new();
    let mut actions: Vec<Action> = Vec::new();
    let mut block = Block::None;
    // The summary is the first indented line of an action block, even when it is `?`.
    let mut summary_read: Vec<bool> = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        if raw.contains('\t') {
            diags.push(diag(n, "tab character: indent with spaces"));
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            block = Block::None;
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let indented = raw.starts_with(' ');

        if !indented {
            block = Block::None;
            let (head, rest) = take(trimmed, 1);
            match head[0] {
                "api" => {
                    if api.is_some() {
                        diags.push(diag(n, "second api line"));
                        continue;
                    }
                    let Some(split) = rest.rfind(char::is_whitespace) else {
                        diags.push(diag(
                            n,
                            "api line needs a name and a base URL: api NAME  BASE_URL",
                        ));
                        continue;
                    };
                    let (name, base) = (rest[..split].trim(), rest[split..].trim());
                    api = Some(Api {
                        name: name.to_string(),
                        base: base.to_string(),
                        line: n,
                    });
                }
                "request" => {
                    if rest.is_empty() {
                        diags.push(diag(n, "request line needs words: request WORDS"));
                    }
                    request = Some(rest.to_string());
                }
                "roles" => {
                    let (toks, extra) = take(rest, 1);
                    match toks.first() {
                        Some(g) if extra.is_empty() => {
                            groups.push(RoleGroup {
                                name: g.to_string(),
                                roles: Vec::new(),
                                line: n,
                            });
                            block = Block::Roles(groups.len() - 1);
                        }
                        _ => diags.push(diag(n, "roles line is: roles GROUP")),
                    }
                }
                word @ ("read" | "change" | "read?" | "change?") => {
                    let (toks, extra) = take(rest, 3);
                    if toks.len() != 3 || !extra.is_empty() {
                        diags.push(diag(n, format!("action line is: {word} NAME  METHOD PATH")));
                        continue;
                    }
                    let kind = if word.starts_with("read") {
                        Kind::Read
                    } else {
                        Kind::Change
                    };
                    actions.push(Action {
                        name: toks[0].to_string(),
                        kind,
                        kind_guessed: word.ends_with('?'),
                        method: toks[1].to_string(),
                        path: toks[2].to_string(),
                        summary: None,
                        inputs: Vec::new(),
                        yes: Vec::new(),
                        no: Vec::new(),
                        line: n,
                    });
                    summary_read.push(false);
                    block = Block::Action(actions.len() - 1);
                }
                other => diags.push(diag(
                    n,
                    format!("unknown line {other:?}: expected api, request, roles, read or change"),
                )),
            }
            continue;
        }

        match block {
            Block::None => diags.push(diag(n, "indented line outside a roles or action block")),
            Block::Roles(g) => {
                let (toks, words) = take(trimmed, 1);
                let head = toks[0];
                let (name, covers) = match head.split_once('=') {
                    Some((name, cov)) => (name, cov.split('+').map(str::to_string).collect()),
                    None => (head, Vec::new()),
                };
                if words.is_empty() {
                    diags.push(diag(n, format!("role {name} has no words")));
                }
                groups[g].roles.push(Role {
                    name: name.to_string(),
                    covers,
                    words: words.to_string(),
                    line: n,
                });
            }
            Block::Action(a) => {
                let action = &mut actions[a];
                if !summary_read[a] {
                    summary_read[a] = true;
                    action.summary = if trimmed == "?" {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                    continue;
                }
                let (toks, rest) = take(trimmed, 1);
                match toks[0] {
                    "yes" => action.yes.push(rest.to_string()),
                    "no" => action.no.push(rest.to_string()),
                    _ => match parse_input(trimmed, n) {
                        Ok(input) => action.inputs.push(input),
                        Err(d) => diags.push(d),
                    },
                }
            }
        }
    }

    if api.is_none() {
        diags.push(diag(1, "missing api line: api NAME  BASE_URL"));
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    Ok(Brev {
        api: api.expect("checked above"),
        request,
        groups,
        actions,
    })
}

fn parse_input(line: &str, n: usize) -> Result<Input, Diag> {
    let (toks, rest) = take(line, 2);
    if toks.len() < 2 {
        return Err(diag(n, "input line is: NAME[?]  TYPE  [GROUP.ROLE]  WORDS"));
    }
    let (name, optional) = match toks[0].strip_suffix('?') {
        Some(base) => (base, true),
        None => (toks[0], false),
    };
    let ty = Type::parse(toks[1]).ok_or_else(|| {
        diag(
            n,
            format!(
                "unknown type {:?}: use str, int, date, weekday, time, ref:ACTION or one:A|B",
                toks[1]
            ),
        )
    })?;
    let (maybe_role, after) = take(rest, 1);
    let (role, words) = match maybe_role.first() {
        Some(tok) if looks_like_role(tok) => {
            let (g, r) = tok
                .split_once('.')
                .expect("looks_like_role checked the dot");
            (Some((g.to_string(), r.to_string())), after)
        }
        _ => (None, rest),
    };
    if words.is_empty() {
        return Err(diag(
            n,
            format!("input {name} has no words: write them, or ? if they are not written yet"),
        ));
    }
    let words = if words == "?" {
        None
    } else {
        Some(words.to_string())
    };
    Ok(Input {
        name: name.to_string(),
        optional,
        ty,
        role,
        words,
        line: n,
    })
}

fn looks_like_role(tok: &str) -> bool {
    match tok.split_once('.') {
        Some((g, r)) => is_ident(g) && is_ident(r),
        None => false,
    }
}

fn words_repeat_name(words: &str, name: &str) -> bool {
    let w: String = words
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    w == name.to_lowercase().replace('_', " ") || w == name.to_lowercase()
}

/// Every problem that stops the file from being used.
pub fn check(b: &Brev) -> Vec<Diag> {
    let mut d = Vec::new();
    if !(b.api.base.starts_with("http://") || b.api.base.starts_with("https://")) {
        d.push(diag(
            b.api.line,
            format!(
                "base URL must start with http:// or https://, not {:?}",
                b.api.base
            ),
        ));
    }

    let mut seen_groups = std::collections::HashSet::new();
    for g in &b.groups {
        if !is_ident(&g.name) {
            d.push(diag(
                g.line,
                format!("role group name {:?} is not an identifier", g.name),
            ));
        }
        if !seen_groups.insert(&g.name) {
            d.push(diag(
                g.line,
                format!("role group {} declared twice", g.name),
            ));
        }
        if g.roles.len() < 2 {
            d.push(diag(
                g.line,
                format!("role group {} needs at least two roles", g.name),
            ));
        }
        let names: Vec<&str> = g.roles.iter().map(|r| r.name.as_str()).collect();
        for (i, r) in g.roles.iter().enumerate() {
            if !is_ident(&r.name) || RESERVED.contains(&r.name.as_str()) {
                d.push(diag(
                    r.line,
                    format!("role name {:?} is not a usable identifier", r.name),
                ));
            }
            if names[..i].contains(&r.name.as_str()) {
                d.push(diag(
                    r.line,
                    format!("role {} declared twice in {}", r.name, g.name),
                ));
            }
            for c in &r.covers {
                if c == &r.name || !names.contains(&c.as_str()) {
                    d.push(diag(
                        r.line,
                        format!(
                            "role {} covers {c:?}, which is not another role in {}",
                            r.name, g.name
                        ),
                    ));
                }
            }
        }
    }

    if b.actions.is_empty() {
        d.push(diag(0, "no actions: add a read or change block"));
    }
    let mut seen_actions = std::collections::HashSet::new();
    for a in &b.actions {
        if !is_ident(&a.name) || RESERVED.contains(&a.name.as_str()) {
            d.push(diag(
                a.line,
                format!("action name {:?} is not a usable identifier", a.name),
            ));
        }
        if !seen_actions.insert(&a.name) {
            d.push(diag(a.line, format!("action {} declared twice", a.name)));
        }
        if a.kind_guessed {
            d.push(diag(
                a.line,
                format!(
                    "{}? is a guess from {}: confirm read or change",
                    a.kind.as_str(),
                    a.method
                ),
            ));
        }
        if !METHODS.contains(&a.method.as_str()) {
            d.push(diag(
                a.line,
                format!("method {:?} is not an HTTP method in capitals", a.method),
            ));
        }
        if !a.path.starts_with('/') {
            d.push(diag(a.line, format!("path {:?} must start with /", a.path)));
        }
        match &a.summary {
            None => d.push(diag(
                a.line + 1,
                format!(
                    "{} has no summary: write one line in the person's words",
                    a.name
                ),
            )),
            Some(s) if s.ends_with("...") || s.ends_with('…') => d.push(diag(
                a.line + 1,
                format!("{}'s summary looks cut short: write it whole", a.name),
            )),
            _ => {}
        }
        for p in a.path_params() {
            if a.input(&p).is_none() {
                d.push(diag(
                    a.line,
                    format!("path names {{{p}}}, which is not an input of {}", a.name),
                ));
            }
        }
        let mut seen_inputs = std::collections::HashSet::new();
        for i in &a.inputs {
            if !is_ident(&i.name) || RESERVED.contains(&i.name.as_str()) {
                d.push(diag(
                    i.line,
                    format!("input name {:?} is not a usable identifier", i.name),
                ));
            }
            if !seen_inputs.insert(&i.name) {
                d.push(diag(
                    i.line,
                    format!("input {} declared twice in {}", i.name, a.name),
                ));
            }
            match &i.words {
                None => d.push(diag(
                    i.line,
                    format!(
                        "{} has no words: Jev cannot be asked whether the request says it",
                        i.name
                    ),
                )),
                Some(w) if words_repeat_name(w, &i.name) => d.push(diag(
                    i.line,
                    format!(
                        "{}'s words repeat its name: say what the person would say",
                        i.name
                    ),
                )),
                _ => {}
            }
            if let Type::Ref(target) = &i.ty {
                match b.action(target) {
                    None => d.push(diag(
                        i.line,
                        format!("ref:{target} names no action in this file"),
                    )),
                    Some(t) if t.kind != Kind::Read => d.push(diag(
                        i.line,
                        format!("ref:{target} must name a read action"),
                    )),
                    _ => {}
                }
            }
            if let Type::OneOf(opts) = &i.ty {
                if opts.len() < 2 {
                    d.push(diag(
                        i.line,
                        format!("{}: one: needs at least two values", i.name),
                    ));
                }
                if opts.iter().any(|o| o == "none_of_these") {
                    d.push(diag(
                        i.line,
                        format!("{}: none_of_these is reserved", i.name),
                    ));
                }
            }
            if let Some((g, r)) = &i.role {
                match b.group(g) {
                    None => d.push(diag(i.line, format!("{}.{r}: no role group {g}", g))),
                    Some(group) => {
                        if !group.roles.iter().any(|x| &x.name == r) {
                            d.push(diag(i.line, format!("{g}.{r}: {g} has no role {r}")));
                        }
                    }
                }
                if !i.ty.has_spotter() {
                    d.push(diag(
                        i.line,
                        format!("{} is in a role group but {} has no spotter: use weekday, time or date", i.name, i.ty),
                    ));
                }
            }
        }
        if a.path_params()
            .iter()
            .any(|p| a.input(p).is_some_and(|i| i.optional))
        {
            d.push(diag(
                a.line,
                format!("{}: an input in the path cannot be optional", a.name),
            ));
        }
    }
    d.sort_by_key(|x| x.line);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = "api Shop  https://shop.example\n\nread find  GET /items/{id}\n  Find an item\n  id  str  which item\n";

    #[test]
    fn minimal_file_loads() {
        let b = load(MIN).unwrap();
        assert_eq!(b.api.name, "Shop");
        assert_eq!(b.api.base, "https://shop.example");
        assert_eq!(b.actions[0].summary.as_deref(), Some("Find an item"));
        assert_eq!(b.actions[0].inputs[0].words.as_deref(), Some("which item"));
    }

    #[test]
    fn unwritten_words_parse_but_fail_check() {
        let t = MIN.replace("which item", "?");
        let b = parse(&t).unwrap();
        assert_eq!(b.actions[0].inputs[0].words, None);
        let d = check(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, 5);
        assert!(d[0].message.contains("no words"));
    }

    #[test]
    fn unwritten_summary_consumes_its_line() {
        let t = MIN.replace("Find an item", "?");
        let b = parse(&t).unwrap();
        assert_eq!(b.actions[0].summary, None);
        assert_eq!(b.actions[0].inputs.len(), 1);
        assert!(b.actions[0].no.is_empty());
    }

    #[test]
    fn guessed_kind_fails_check() {
        let t = MIN.replace("read find", "read? find");
        let d = load(&t).unwrap_err();
        assert!(d[0].message.contains("confirm read or change"));
    }

    #[test]
    fn path_param_must_be_an_input() {
        let t = MIN.replace("/items/{id}", "/items/{item}");
        let d = load(&t).unwrap_err();
        assert!(d.iter().any(|x| x.message.contains("{item}")));
    }

    #[test]
    fn words_repeating_the_name_are_refused() {
        let t = MIN.replace("which item", "id");
        let d = load(&t).unwrap_err();
        assert!(d[0].message.contains("repeat its name"));
    }

    #[test]
    fn tabs_and_unknown_lines_are_errors_with_lines() {
        let d = parse("api X  https://x\nfoo bar\n\tbad\n").unwrap_err();
        assert_eq!(d.iter().map(|x| x.line).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn role_inputs_need_a_spotter_type_and_a_real_role() {
        let t = "api X  https://x\nroles g\n  a  the {kind} a\n  b  the {kind} b\n\nchange m  POST /m\n  Move it\n  x  str  g.a  which x\n  y  time  g.c  which y\n";
        let d = load(t).unwrap_err();
        assert!(
            d.iter()
                .any(|x| x.line == 8 && x.message.contains("no spotter"))
        );
        assert!(
            d.iter()
                .any(|x| x.line == 9 && x.message.contains("has no role c"))
        );
    }

    #[test]
    fn covers_must_name_other_roles() {
        let t = "api X  https://x\nroles g\n  a  the {kind} a\n  b=a+z  the {kind} b\n\nread r  GET /r\n  Read it\n";
        let d = load(t).unwrap_err();
        assert!(d[0].message.contains("covers \"z\""));
    }

    #[test]
    fn truncated_summary_is_refused() {
        let t = MIN.replace("Find an item", "Find an item by...");
        assert!(load(&t).unwrap_err()[0].message.contains("cut short"));
    }
}
