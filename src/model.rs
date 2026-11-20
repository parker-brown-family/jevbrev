//! The parsed form of a `.brev` file.

use std::fmt;

/// A whole `.brev` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Brev {
    pub api: Api,
    /// What the `request` field in `state` holds. Documentation only; Jev never sees it.
    pub request: Option<String>,
    pub groups: Vec<RoleGroup>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Api {
    pub name: String,
    pub base: String,
    pub line: usize,
}

/// Roles that compete for the same words in a request.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleGroup {
    pub name: String,
    pub roles: Vec<Role>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    pub name: String,
    /// Other roles in the group this one also fills (`stays=now+moves_to`).
    pub covers: Vec<String>,
    pub words: String,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Read,
    Change,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Read => "read",
            Kind::Change => "change",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub name: String,
    pub kind: Kind,
    /// `read?` or `change?`: guessed by `import` from the HTTP method, not yet confirmed.
    pub kind_guessed: bool,
    pub method: String,
    pub path: String,
    /// `None` when the summary is unwritten (`?`).
    pub summary: Option<String>,
    pub inputs: Vec<Input>,
    pub yes: Vec<String>,
    pub no: Vec<String>,
    pub line: usize,
}

impl Action {
    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|i| i.name == name)
    }

    /// Names in `{braces}` in the path.
    pub fn path_params(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = self.path.as_str();
        while let Some(start) = rest.find('{') {
            let after = &rest[start + 1..];
            match after.find('}') {
                Some(end) => {
                    out.push(after[..end].to_string());
                    rest = &after[end + 1..];
                }
                None => break,
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    pub optional: bool,
    pub ty: Type,
    /// `(group, role)` when the input takes its value from a role group.
    pub role: Option<(String, String)>,
    /// `None` when the words are unwritten (`?`).
    pub words: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Str,
    Int,
    Date,
    Weekday,
    Time,
    /// The value is one of the results of another `read` action.
    Ref(String),
    /// The value is one of a fixed list (`one:keep|reschedule`).
    OneOf(Vec<String>),
}

impl Type {
    pub fn parse(s: &str) -> Option<Type> {
        Some(match s {
            "str" => Type::Str,
            "int" => Type::Int,
            "date" => Type::Date,
            "weekday" => Type::Weekday,
            "time" => Type::Time,
            _ if s.starts_with("one:") => {
                let opts: Vec<String> = s[4..].split('|').map(str::to_string).collect();
                if opts.iter().any(String::is_empty) {
                    return None;
                }
                Type::OneOf(opts)
            }
            _ => {
                let target = s.strip_prefix("ref:")?;
                if target.is_empty() {
                    return None;
                }
                Type::Ref(target.to_string())
            }
        })
    }

    /// The noun that replaces `{kind}` in a role's words, for types with a spotter.
    pub fn noun(&self) -> Option<&'static str> {
        match self {
            Type::Weekday => Some("weekday"),
            Type::Time => Some("time"),
            Type::Date => Some("date"),
            _ => None,
        }
    }

    pub fn has_spotter(&self) -> bool {
        self.noun().is_some()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Str => f.write_str("str"),
            Type::Int => f.write_str("int"),
            Type::Date => f.write_str("date"),
            Type::Weekday => f.write_str("weekday"),
            Type::Time => f.write_str("time"),
            Type::Ref(a) => write!(f, "ref:{a}"),
            Type::OneOf(o) => write!(f, "one:{}", o.join("|")),
        }
    }
}

/// One problem in a `.brev` file, with the line it is on (1-based; 0 means the whole file).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diag {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}  {}", self.line, self.message)
    }
}

impl Brev {
    pub fn action(&self, name: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.name == name)
    }

    pub fn group(&self, name: &str) -> Option<&RoleGroup> {
        self.groups.iter().find(|g| g.name == name)
    }
}
