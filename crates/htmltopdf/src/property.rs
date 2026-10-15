//! `@property` initial values, which the engine does not apply.
//!
//! Stylo (as used by Blitz) does not honour registered custom properties, so a property declared with
//! `@property --x { inherits: false; initial-value: solid }` stays unset unless some rule sets it. Tailwind
//! v4 relies on this for every border style, gradient stop, shadow and ring default, and its own fallback
//! block is gated behind an `@supports` that Stylo does not match. See
//! `docs/plans/2026-10-15-f2-property-defaults.md`.
//!
//! We emulate the registration with a generated stylesheet in the first-declared (lowest-priority)
//! cascade layer: non-inherited properties are set on every element, inherited ones on the root. Any
//! layered or unlayered author declaration overrides it, as it would override a real initial value.
//! A non-inherited property without an initial value gets `initial` (the guaranteed-invalid value), so a
//! child falls back in `var(--x, fallback)` instead of inheriting its parent's value.

use cssparser::{ParseError, Parser, ParserInput, Token};

/// Name of the cascade layer that holds the generated defaults. It must be the first layer declared.
pub(crate) const LAYER: &str = "htmltopdf-property-defaults";

/// A valid `@property` rule (css-properties-values-api-1 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PropertyRule {
    pub(crate) name: String,
    pub(crate) syntax: String,
    pub(crate) inherits: bool,
    /// Raw `initial-value` text, trimmed. `None` only for the universal syntax `*`.
    pub(crate) initial: Option<String>,
}

/// Registered properties in registration order; a later valid rule for the same name replaces the
/// earlier one ("last one wins").
#[derive(Debug, Clone, Default)]
pub(crate) struct Registry {
    rules: Vec<PropertyRule>,
}

impl Registry {
    pub(crate) fn extend(&mut self, rules: Vec<PropertyRule>) {
        for rule in rules {
            match self.rules.iter_mut().find(|r| r.name == rule.name) {
                Some(existing) => *existing = rule,
                None => self.rules.push(rule),
            }
        }
    }

    /// The defaults stylesheet, or `None` when nothing is registered (no change to the output).
    pub(crate) fn prelude(&self) -> Option<String> {
        if self.rules.is_empty() {
            return None;
        }
        let mut non_inherited = String::new();
        let mut inherited = String::new();
        for r in &self.rules {
            let value = match (&r.initial, r.inherits) {
                (Some(v), _) => computed_initial(&r.syntax, v),
                // Inherited without an initial value behaves like an unregistered property.
                (None, true) => continue,
                (None, false) => "initial".to_string(),
            };
            let out = if r.inherits {
                &mut inherited
            } else {
                &mut non_inherited
            };
            out.push_str(&format!("{}:{};", r.name, value));
        }
        let mut css = format!("@layer {LAYER}{{");
        if !non_inherited.is_empty() {
            css.push_str(&format!("*,::before,::after{{{non_inherited}}}"));
        }
        if !inherited.is_empty() {
            css.push_str(&format!(":root{{{inherited}}}"));
        }
        css.push_str("}\n");
        Some(css)
    }
}

/// A registered `<length>` substitutes its computed value, so `initial-value: 0` reaches `var()` as `0px`.
/// A bare `0` would make `calc(1px + var(--x))` invalid. This is the only computed-value normalisation we do.
fn computed_initial(syntax: &str, value: &str) -> String {
    let lengthy = syntax
        .split('|')
        .map(str::trim)
        .any(|s| s == "<length>" || s == "<length-percentage>");
    if lengthy && value.parse::<f64>().is_ok_and(|n| n == 0.0) {
        "0px".to_string()
    } else {
        value.to_string()
    }
}

/// Valid `@property` rules of a stylesheet, in document order: top level and inside `@layer` blocks.
/// Rules inside conditional group rules (`@media`, `@supports`, …) are not collected.
pub(crate) fn scan_property_rules(css: &str) -> Vec<PropertyRule> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut out = Vec::new();
    scan_rules(&mut parser, &mut out);
    out
}

fn scan_rules(p: &mut Parser<'_, '_>, out: &mut Vec<PropertyRule>) {
    loop {
        let tok = match p.next() {
            Ok(t) => t.clone(),
            Err(_) => return,
        };
        match tok {
            Token::AtKeyword(kw) if kw.eq_ignore_ascii_case("property") => {
                let name = match p.next() {
                    Ok(Token::Ident(n)) if n.starts_with("--") => Some(n.to_string()),
                    _ => None,
                };
                let Some(block) = next_block(p) else { return };
                if !block {
                    continue;
                }
                let rule = p
                    .parse_nested_block(|p| -> Result<_, ParseError<'_, ()>> {
                        Ok(parse_descriptors(p))
                    })
                    .ok()
                    .and_then(|d| validate(name, d));
                out.extend(rule);
            }
            Token::AtKeyword(kw) if kw.eq_ignore_ascii_case("layer") => {
                // `@layer a, b;` (statement) or `@layer a { … }` (block).
                let Some(block) = next_block(p) else { return };
                if block {
                    let _ = p.parse_nested_block(|p| -> Result<(), ParseError<'_, ()>> {
                        scan_rules(p, out);
                        Ok(())
                    });
                }
            }
            // Any other block (style rule, `@media`, `@supports`, `@font-face` …) is skipped whole by
            // `next()`; its prelude tokens are skipped one by one.
            _ => {}
        }
    }
}

/// Advances to the end of an at-rule prelude. `Some(true)` when positioned on a `{}` block (call
/// `parse_nested_block` next), `Some(false)` after a `;`, `None` at the end of input.
fn next_block(p: &mut Parser<'_, '_>) -> Option<bool> {
    loop {
        match p.next() {
            Ok(Token::CurlyBracketBlock) => return Some(true),
            Ok(Token::Semicolon) => return Some(false),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

#[derive(Default)]
struct Descriptors {
    syntax: Option<String>,
    inherits: Option<bool>,
    initial: Option<String>,
    /// A descriptor was present but its value is invalid → the whole rule is invalid.
    invalid: bool,
}

fn parse_descriptors(p: &mut Parser<'_, '_>) -> Descriptors {
    let mut d = Descriptors::default();
    loop {
        let name = match p.next() {
            Ok(Token::Ident(n)) => n.to_ascii_lowercase(),
            Ok(_) => continue,
            Err(_) => return d,
        };
        if p.try_parse(|p| p.expect_colon()).is_err() {
            continue;
        }
        let start = p.position();
        while p.next().is_ok_and(|t| !matches!(t, Token::Semicolon)) {}
        let raw = p.slice_from(start).trim().trim_end_matches(';').trim();
        match name.as_str() {
            "syntax" => match unquote(raw) {
                Some(s) => d.syntax = Some(s.trim().to_string()),
                None => d.invalid = true,
            },
            "inherits" => match raw.to_ascii_lowercase().as_str() {
                "true" => d.inherits = Some(true),
                "false" => d.inherits = Some(false),
                _ => d.invalid = true,
            },
            "initial-value" => d.initial = Some(raw.to_string()),
            _ => {}
        }
    }
}

/// `syntax` is a `<string>`.
fn unquote(raw: &str) -> Option<&str> {
    let q = raw.chars().next()?;
    if !matches!(q, '"' | '\'') || raw.len() < 2 || !raw.ends_with(q) {
        return None;
    }
    Some(&raw[1..raw.len() - 1])
}

/// css-properties-values-api-1 §2: `syntax` and `inherits` are required; `initial-value` is required unless
/// the syntax is the universal `*`.
fn validate(name: Option<String>, d: Descriptors) -> Option<PropertyRule> {
    if d.invalid {
        return None;
    }
    let name = name?;
    let syntax = d.syntax?;
    let inherits = d.inherits?;
    let initial = d.initial.filter(|v| !v.is_empty());
    if syntax != "*" && initial.is_none() {
        return None;
    }
    Some(PropertyRule {
        name,
        syntax,
        inherits,
        initial,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(css: &str) -> Vec<String> {
        scan_property_rules(css)
            .into_iter()
            .map(|r| r.name)
            .collect()
    }

    #[test]
    fn parses_descriptors() {
        let rules = scan_property_rules(
            r#"@property --a { syntax: "<length>"; inherits: false; initial-value: 0 }
               @property --b{syntax:'*';inherits:TRUE}"#,
        );
        assert_eq!(
            rules,
            vec![
                PropertyRule {
                    name: "--a".into(),
                    syntax: "<length>".into(),
                    inherits: false,
                    initial: Some("0".into()),
                },
                PropertyRule {
                    name: "--b".into(),
                    syntax: "*".into(),
                    inherits: true,
                    initial: None,
                },
            ]
        );
    }

    #[test]
    fn initial_value_keeps_commas_and_functions() {
        let r = scan_property_rules(
            r#"@property --s { syntax: "*"; inherits: false; initial-value: 0 0 #0000, 1px 2px rgb(0 0 0 / .5) }"#,
        );
        assert_eq!(
            r[0].initial.as_deref(),
            Some("0 0 #0000, 1px 2px rgb(0 0 0 / .5)")
        );
    }

    #[test]
    fn invalid_rules_are_ignored() {
        let css = r#"
            @property --no-inherits { syntax: "*"; initial-value: 1 }
            @property --no-syntax { inherits: false; initial-value: 1 }
            @property --typed-without-initial { syntax: "<color>"; inherits: false }
            @property --bad-inherits { syntax: "*"; inherits: maybe }
            @property --unquoted-syntax { syntax: *; inherits: false }
            @property not-dashed { syntax: "*"; inherits: false }
            @property --ok { syntax: "*"; inherits: false }
        "#;
        assert_eq!(names(css), ["--ok"]);
    }

    #[test]
    fn found_at_top_level_and_in_layers_not_in_conditional_rules() {
        let css = r#"
            @layer a, b;
            .x { --in-style-rule: 1 }
            @layer a { @property --in-layer { syntax: "*"; inherits: false } }
            @media print { @property --in-media { syntax: "*"; inherits: false } }
            @supports (color: red) { @property --in-supports { syntax: "*"; inherits: false } }
            @property --top { syntax: "*"; inherits: false }
        "#;
        assert_eq!(names(css), ["--in-layer", "--top"]);
    }

    #[test]
    fn last_valid_rule_wins_and_keeps_first_position() {
        let mut reg = Registry::default();
        reg.extend(scan_property_rules(
            r#"@property --a { syntax: "*"; inherits: false; initial-value: 1 }
               @property --b { syntax: "*"; inherits: false; initial-value: 2 }"#,
        ));
        reg.extend(scan_property_rules(
            r#"@property --a { syntax: "*"; inherits: false; initial-value: 3 }"#,
        ));
        assert_eq!(
            reg.prelude().unwrap(),
            "@layer htmltopdf-property-defaults{*,::before,::after{--a:3;--b:2;}}\n"
        );
    }

    #[test]
    fn prelude_splits_inherited_and_non_inherited() {
        let mut reg = Registry::default();
        reg.extend(scan_property_rules(
            r#"@property --style { syntax: "*"; inherits: false; initial-value: solid }
               @property --leading { syntax: "*"; inherits: false }
               @property --ink { syntax: "<color>"; inherits: true; initial-value: #111 }
               @property --plain { syntax: "*"; inherits: true }
               @property --w { syntax: "<length>"; inherits: false; initial-value: 0 }
               @property --pos { syntax: "<length-percentage>"; inherits: false; initial-value: 0% }"#,
        ));
        assert_eq!(
            reg.prelude().unwrap(),
            "@layer htmltopdf-property-defaults{\
             *,::before,::after{--style:solid;--leading:initial;--w:0px;--pos:0%;}\
             :root{--ink:#111;}}\n"
        );
    }

    #[test]
    fn no_rules_no_prelude() {
        assert_eq!(Registry::default().prelude(), None);
    }

    #[test]
    fn tailwind_stylesheet() {
        let css = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/tailwind/tailwind.css"
        ))
        .unwrap();
        let rules = scan_property_rules(&css);
        let get = |n: &str| rules.iter().find(|r| r.name == n).unwrap();
        assert!(rules.len() >= 30, "{} rules", rules.len());
        assert!(rules.iter().all(|r| !r.inherits));
        assert_eq!(get("--tw-border-style").initial.as_deref(), Some("solid"));
        assert_eq!(get("--tw-leading").initial, None);
        assert_eq!(get("--tw-shadow").initial.as_deref(), Some("0 0 #0000"));
        // Tailwind's own `@supports` fallback writes `0px` here; the computed value of a `<length>` 0.
        let mut reg = Registry::default();
        reg.extend(rules);
        assert!(
            reg.prelude()
                .unwrap()
                .contains("--tw-ring-offset-width:0px;")
        );
    }
}
