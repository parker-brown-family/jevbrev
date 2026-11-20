//! Spotters: code that finds candidate words in a request for the types that have them.
//!
//! A spotter over-finds on purpose. Jev decides which of the spans matter; jevbrev never
//! invents or rewrites a value, it returns a span of the request.

use crate::model::Type;
use regex::Regex;
use std::sync::OnceLock;

/// A candidate value found in the request: the text, and where it is (byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

fn weekday_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?-u:\b)((?:mon|tues|wednes|thurs|fri|satur|sun)day)s?(?-u:\b)")
            .expect("valid regex")
    })
}

fn time_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // 9, 9:30, 9.30, 9am, 9:30 pm, and the plural forms ("her 9:30s"), never part of a longer number.
    R.get_or_init(|| {
        Regex::new(r"(?:^|[^0-9a-z_:./-])([0-9]{1,2}(?:[:.][0-9]{2})?(?: ?[ap]m)?)s?(?-u:\b)")
            .expect("valid regex")
    })
}

fn date_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(concat!(
            r"(?-u:\b)(",
            r"[0-9]{4}-[0-9]{2}-[0-9]{2}",
            r"|[0-9]{1,2}/[0-9]{1,2}(?:/[0-9]{2,4})?",
            r"|(?:january|february|march|april|may|june|july|august|september|october|november|december",
            r"|jan|feb|mar|apr|jun|jul|aug|sept|sep|oct|nov|dec)\.?(?: +[0-9]{1,2}(?:st|nd|rd|th)?)?",
            r"|(?:the +)?[0-9]{1,2}(?:st|nd|rd|th)(?: +of +[a-z]+)?",
            r"|(?:next|this) +(?:week|month|year)",
            r"|(?:(?:next|this) +)?(?:mon|tues|wednes|thurs|fri|satur|sun)day",
            r"|today|tomorrow",
            r")(?-u:\b)"
        ))
        .expect("valid regex")
    })
}

/// The patterns are written in lower case and matched against an ASCII-lower-cased copy, which
/// has the same byte offsets, so each span is cut from the original text as the person wrote it.
fn find(re: &Regex, text: &str) -> Vec<Span> {
    let lower = text.to_ascii_lowercase();
    re.captures_iter(&lower)
        .filter_map(|c| c.get(1))
        .map(|m| Span {
            text: text[m.start()..m.end()].to_string(),
            start: m.start(),
            end: m.end(),
        })
        .collect()
}

/// Every span of `text` the spotter for `ty` finds, in order. Types without a spotter find nothing.
///
/// Times that sit inside a date (`2026-11-20`, `11/20`) are dropped, so a date is never also
/// offered as a time.
pub fn spot(ty: &Type, text: &str) -> Vec<Span> {
    match ty {
        Type::Weekday => find(weekday_re(), text),
        Type::Date => find(date_re(), text),
        Type::Time => {
            let dates = find(date_re(), text);
            find(time_re(), text)
                .into_iter()
                .filter(|t| !dates.iter().any(|d| t.start < d.end && d.start < t.end))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// How spans are compared when two candidates claim the same input: lower case, a plural `s`
/// and a whole-hour `:00` dropped. `Wednesdays`, `wednesday` and `Wednesday` are one value, and
/// so are `10:00` and `10`.
pub fn normalise(v: &str) -> String {
    let mut s = v.trim().to_lowercase();
    if s.ends_with('s') && s.len() > 1 && !s.ends_with("ss") {
        s.pop();
    }
    if let Some(stripped) = s.strip_suffix(":00") {
        s = stripped.to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(ty: Type, s: &str) -> Vec<String> {
        spot(&ty, s).into_iter().map(|x| x.text).collect()
    }

    #[test]
    fn weekdays_drop_the_plural() {
        assert_eq!(
            texts(Type::Weekday, "move his Mondays to Thursday"),
            ["Monday", "Thursday"]
        );
        assert_eq!(texts(Type::Weekday, "on wednesdays"), ["wednesday"]);
        assert!(texts(Type::Weekday, "a day off").is_empty());
    }

    #[test]
    fn times_are_kept_whole() {
        // The measured run cut "9:30s" to "9"; this is the case that must not happen again.
        assert_eq!(
            texts(Type::Time, "move his current Monday 9:30s to Monday 8:30"),
            ["9:30", "8:30"]
        );
        assert_eq!(
            texts(
                Type::Time,
                "move Alex's Tuesday 10:00s to Thursday at 14:30"
            ),
            ["10:00", "14:30"]
        );
        assert_eq!(
            texts(Type::Time, "at 3pm or 4 pm, or 9.15"),
            ["3pm", "4 pm", "9.15"]
        );
        assert_eq!(
            texts(Type::Time, "bump Jordan from 1:15 to 2:15"),
            ["1:15", "2:15"]
        );
    }

    #[test]
    fn times_are_not_found_inside_dates_or_ordinals() {
        assert!(texts(Type::Time, "from the 8th").is_empty());
        assert!(texts(Type::Time, "on 2026-11-20").is_empty());
        assert!(texts(Type::Time, "on 11/20").is_empty());
        assert!(texts(Type::Time, "in 2026").is_empty());
    }

    #[test]
    fn dates_over_find() {
        assert_eq!(texts(Type::Date, "from next month"), ["next month"]);
        assert_eq!(
            texts(Type::Date, "moving to Tuesday, from the 8th"),
            ["Tuesday", "the 8th"]
        );
        assert_eq!(texts(Type::Date, "take next Friday off"), ["next Friday"]);
        assert_eq!(texts(Type::Date, "from January"), ["January"]);
        assert_eq!(texts(Type::Date, "starting Dec 3rd"), ["Dec 3rd"]);
        assert_eq!(
            texts(Type::Date, "on 2026-12-01 or 12/1"),
            ["2026-12-01", "12/1"]
        );
    }

    #[test]
    fn normalise_merges_equal_values() {
        assert_eq!(normalise("Wednesdays"), normalise("wednesday"));
        assert_eq!(normalise("10:00"), normalise("10"));
        assert_ne!(normalise("9:30"), normalise("9"));
    }
}
