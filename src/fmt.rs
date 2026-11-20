//! Writing a `Brev` back out as a `.brev` file, in the standard layout.

use crate::model::*;

fn pad(s: &str, w: usize) -> String {
    format!("{s:w$}")
}

/// The file for `b`. `header` lines (comments, without their newline) go first.
pub fn to_text(b: &Brev, header: &[String]) -> String {
    let mut out = String::new();
    for h in header {
        out.push_str(h);
        out.push('\n');
    }
    if !header.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!("api {}  {}\n", b.api.name, b.api.base));
    if let Some(r) = &b.request {
        out.push_str(&format!("request  {r}\n"));
    }

    for g in &b.groups {
        out.push_str(&format!("\nroles {}\n", g.name));
        let heads: Vec<String> = g
            .roles
            .iter()
            .map(|r| {
                if r.covers.is_empty() {
                    r.name.clone()
                } else {
                    format!("{}={}", r.name, r.covers.join("+"))
                }
            })
            .collect();
        let w = heads.iter().map(String::len).max().unwrap_or(0) + 2;
        for (h, r) in heads.iter().zip(&g.roles) {
            out.push_str(&format!("  {}{}\n", pad(h, w), r.words));
        }
    }

    for a in &b.actions {
        let kind = format!(
            "{}{}",
            a.kind.as_str(),
            if a.kind_guessed { "?" } else { "" }
        );
        out.push_str(&format!("\n{kind} {}  {} {}\n", a.name, a.method, a.path));
        out.push_str(&format!("  {}\n", a.summary.as_deref().unwrap_or("?")));
        let names: Vec<String> = a
            .inputs
            .iter()
            .map(|i| format!("{}{}", i.name, if i.optional { "?" } else { "" }))
            .collect();
        let types: Vec<String> = a.inputs.iter().map(|i| i.ty.to_string()).collect();
        let roles: Vec<String> = a
            .inputs
            .iter()
            .map(|i| {
                i.role
                    .as_ref()
                    .map(|(g, r)| format!("{g}.{r}"))
                    .unwrap_or_default()
            })
            .collect();
        let w0 = names.iter().map(String::len).max().unwrap_or(0) + 2;
        let w1 = types.iter().map(String::len).max().unwrap_or(0) + 2;
        let w2 = roles.iter().map(String::len).max().unwrap_or(0);
        for (k, i) in a.inputs.iter().enumerate() {
            let role = if w2 > 0 {
                pad(&roles[k], w2 + 2)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "  {}{}{}{}\n",
                pad(&names[k], w0),
                pad(&types[k], w1),
                role,
                i.words.as_deref().unwrap_or("?")
            ));
        }
        for y in &a.yes {
            out.push_str(&format!("  yes {y}\n"));
        }
        for n in &a.no {
            out.push_str(&format!("  no  {n}\n"));
        }
    }
    out
}

/// The comment lines at the end of a file, after its last other line (blank lines kept).
pub fn trailing_comments(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let last = lines
        .iter()
        .rposition(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    match last {
        Some(i) if i + 1 < lines.len() => {
            let tail: Vec<String> = lines[i + 1..].iter().map(|s| s.to_string()).collect();
            if tail.iter().any(|l| l.trim_start().starts_with('#')) {
                tail
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// The comment lines at the top of a file, before its first other line.
pub fn leading_comments(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|l| l.trim_start().starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn the_example_round_trips() {
        let text = include_str!("../examples/clinic.brev");
        let b = parse(text).unwrap();
        let again = parse(&to_text(&b, &[])).unwrap();
        // Line numbers differ; everything else is the same.
        let strip = |b: &Brev| {
            let mut b = b.clone();
            b.api.line = 0;
            for g in &mut b.groups {
                g.line = 0;
                for r in &mut g.roles {
                    r.line = 0;
                }
            }
            for a in &mut b.actions {
                a.line = 0;
                for i in &mut a.inputs {
                    i.line = 0;
                }
            }
            b
        };
        assert_eq!(strip(&b), strip(&again));
    }

    #[test]
    fn unwritten_and_guessed_stay_visible() {
        let b = parse("api X  https://x\n\nchange? m  POST /m/{id}\n  ?\n  id  str  ?\n").unwrap();
        let t = to_text(&b, &["# hello".into()]);
        assert!(t.starts_with("# hello\n\napi X  https://x\n"));
        assert!(t.contains("change? m  POST /m/{id}\n  ?\n  id  str  ?\n"));
    }
}
