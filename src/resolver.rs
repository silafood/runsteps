//! Placeholder interpolation for `args` and `env` values (US-008).
//!
//! Syntax: `{{name}}` — resolved from:
//!   1. `--var name=value` CLI flags
//!   2. `prompts[name]` via `inquire::Text` (interactive only)
//!   3. Hard error (exit 2) — no silent empty substitution
//!
//! Literal `{{` is written as `{{{{`; `}}` as `}}}}`.
//! Values containing `\n` or `\r` are rejected.
//! For `command` steps, values are shell-escaped unless `raw = true`.
//! For `just_recipe` steps, resolved values are passed as argv (no shell escape).

use anyhow::Result;
use std::collections::HashMap;

/// Parse `--var key=value` entries into a map. Returns Err on malformed entries.
pub fn parse_var_flags(vars: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for v in vars {
        match v.split_once('=') {
            Some((k, val)) => {
                let k = k.trim().to_string();
                if k.is_empty() {
                    anyhow::bail!("invalid --var format '{}': key must not be empty", v);
                }
                map.insert(k, val.to_string());
            }
            None => anyhow::bail!(
                "invalid --var format '{}': expected key=value",
                v
            ),
        }
    }
    Ok(map)
}

/// Resolve all `{{placeholder}}` occurrences in `template`.
///
/// - `{{{{` → `{{`
/// - `}}}}` → `}}`
/// - `{{name}}` → looked up from `vars`, then `prompts` (interactive prompt), else hard error.
/// - Values with `\n` or `\r` are rejected.
/// - If `shell_escape_values` is true, resolved values are shell-escaped (for command steps).
pub fn resolve_placeholders(
    template: &str,
    step_name: &str,
    vars: &HashMap<String, String>,
    prompts: &HashMap<String, String>,
    yes_mode: bool,
    shell_escape_values: bool,
) -> Result<String> {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                // Check for escaped {{{{ → {
                if chars.peek() == Some(&'{') {
                    chars.next(); // consume third '{'
                    if chars.peek() == Some(&'{') {
                        chars.next(); // consume fourth '{'
                        result.push('{');
                        result.push('{');
                    } else {
                        // Just {{{ — treat first two as literal {{ and re-examine rest
                        result.push('{');
                        result.push('{');
                        result.push('{');
                    }
                } else {
                    // Start of placeholder {{name}}
                    let mut name = String::new();
                    let mut closed = false;
                    while let Some(c) = chars.next() {
                        if c == '}' {
                            if chars.peek() == Some(&'}') {
                                chars.next(); // consume closing '}'
                                closed = true;
                                break;
                            } else {
                                name.push(c);
                            }
                        } else {
                            name.push(c);
                        }
                    }
                    if !closed {
                        // Unclosed placeholder — treat literally
                        result.push_str("{{");
                        result.push_str(&name);
                        continue;
                    }
                    let value = resolve_single(step_name, &name, vars, prompts, yes_mode)?;
                    validate_value(&value, step_name, &name)?;
                    if shell_escape_values {
                        let escaped = shell_escape::unix::escape(std::borrow::Cow::Borrowed(&value));
                        result.push_str(&escaped);
                    } else {
                        result.push_str(&value);
                    }
                }
            } else {
                result.push(ch);
            }
        } else if ch == '}' {
            if chars.peek() == Some(&'}') {
                chars.next();
                // Check for }}}} → }}
                if chars.peek() == Some(&'}') {
                    chars.next();
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        result.push('}');
                        result.push('}');
                    } else {
                        result.push('}');
                        result.push('}');
                        result.push('}');
                    }
                } else {
                    // Just }} — literal (closing without opening); pass through
                    result.push('}');
                    result.push('}');
                }
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Resolve a single placeholder name from vars → prompts → error.
fn resolve_single(
    step_name: &str,
    name: &str,
    vars: &HashMap<String, String>,
    prompts: &HashMap<String, String>,
    yes_mode: bool,
) -> Result<String> {
    // 1. CLI --var flag
    if let Some(val) = vars.get(name) {
        return Ok(val.clone());
    }

    // 2. prompts entry (interactive)
    if let Some(prompt_text) = prompts.get(name) {
        if yes_mode {
            anyhow::bail!(
                "error: step '{}' requires value for placeholder '{{{{{}}}}}'; pass --var {}=<value>",
                step_name,
                name,
                name
            );
        }
        let value = inquire::Text::new(prompt_text).prompt()?;
        return Ok(value);
    }

    // 3. Hard error
    if yes_mode {
        anyhow::bail!(
            "error: step '{}' requires value for placeholder '{{{{{}}}}}'; pass --var {}=<value>",
            step_name,
            name,
            name
        );
    }
    // Interactive mode with no prompts entry — still error
    anyhow::bail!(
        "error: step '{}' requires value for placeholder '{{{{{}}}}}'; pass --var {}=<value> or add a prompts entry",
        step_name,
        name,
        name
    );
}

/// Reject values containing newlines.
fn validate_value(value: &str, step_name: &str, placeholder: &str) -> Result<()> {
    if value.contains('\n') || value.contains('\r') {
        anyhow::bail!(
            "error: value for placeholder '{{{{{}}}}}' in step '{}' contains a newline, which is not allowed",
            placeholder,
            step_name
        );
    }
    Ok(())
}

/// Warn about orphan prompts entries (keys not referenced in any args element).
/// Respects `RUNSTEPS_NO_WARNINGS`.
pub fn warn_orphan_prompts(step_name: &str, args: &[String], prompts: &HashMap<String, String>) {
    if std::env::var("RUNSTEPS_NO_WARNINGS").as_deref() == Ok("1") {
        return;
    }
    for key in prompts.keys() {
        let referenced = args.iter().any(|a| a.contains(&format!("{{{{{}}}}}", key)));
        if !referenced {
            eprintln!(
                "warning: prompts.{} in step '{}' is not referenced in any args element",
                key, step_name
            );
        }
    }
}

/// Resolve all args elements for a step, returning the resolved strings.
pub fn resolve_args(
    step_name: &str,
    args: &[String],
    prompts: &HashMap<String, String>,
    vars: &HashMap<String, String>,
    yes_mode: bool,
    shell_escape_values: bool,
) -> Result<Vec<String>> {
    args.iter()
        .map(|a| resolve_placeholders(a, step_name, vars, prompts, yes_mode, shell_escape_values))
        .collect()
}

/// Resolve all env value strings for a step (no shell escaping for env).
pub fn resolve_env(
    step_name: &str,
    env: &HashMap<String, String>,
    prompts: &HashMap<String, String>,
    vars: &HashMap<String, String>,
    yes_mode: bool,
) -> Result<HashMap<String, String>> {
    env.iter()
        .map(|(k, v)| {
            let resolved =
                resolve_placeholders(v, step_name, vars, prompts, yes_mode, false)?;
            Ok((k.clone(), resolved))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_vars() -> HashMap<String, String> {
        HashMap::new()
    }
    fn empty_prompts() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn test_no_placeholders() {
        let result = resolve_placeholders(
            "hello world",
            "step",
            &empty_vars(),
            &empty_prompts(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_single_placeholder_from_vars() {
        let mut vars = HashMap::new();
        vars.insert("pod".to_string(), "web".to_string());
        let result = resolve_placeholders(
            "--pod={{pod}}",
            "step",
            &vars,
            &empty_prompts(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(result, "--pod=web");
    }

    #[test]
    fn test_multi_placeholder_per_element() {
        let mut vars = HashMap::new();
        vars.insert("p".to_string(), "web".to_string());
        vars.insert("e".to_string(), "staging".to_string());
        let result = resolve_placeholders(
            "--pod={{p}}-{{e}}",
            "step",
            &vars,
            &empty_prompts(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(result, "--pod=web-staging");
    }

    #[test]
    fn test_escaped_double_brace() {
        let result = resolve_placeholders(
            "literal {{{{braces}}}}",
            "step",
            &empty_vars(),
            &empty_prompts(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(result, "literal {{braces}}");
    }

    #[test]
    fn test_missing_value_in_yes_mode_errors() {
        let result = resolve_placeholders(
            "{{missing}}",
            "mystep",
            &empty_vars(),
            &empty_prompts(),
            true,
            false,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("missing"), "expected placeholder name in error: {msg}");
    }

    #[test]
    fn test_newline_in_value_rejected() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), "a\nb".to_string());
        let result = resolve_placeholders(
            "{{x}}",
            "step",
            &vars,
            &empty_prompts(),
            true,
            false,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("newline"), "expected 'newline' in error: {msg}");
    }

    #[test]
    fn test_shell_escape_applied() {
        let mut vars = HashMap::new();
        vars.insert("v".to_string(), "hello world".to_string());
        let result = resolve_placeholders(
            "{{v}}",
            "step",
            &vars,
            &empty_prompts(),
            true,
            true, // shell_escape_values = true
        )
        .unwrap();
        // shell_escape wraps in single quotes or similar
        assert!(result.contains("hello") && result.contains("world"));
        assert_ne!(result, "hello world"); // must be escaped
    }

    #[test]
    fn test_raw_no_escape() {
        let mut vars = HashMap::new();
        vars.insert("v".to_string(), "hello world".to_string());
        let result = resolve_placeholders(
            "{{v}}",
            "step",
            &vars,
            &empty_prompts(),
            true,
            false, // raw = no escape
        )
        .unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_parse_var_flags_valid() {
        let vars = parse_var_flags(&["pod=web".to_string(), "env=staging".to_string()]).unwrap();
        assert_eq!(vars.get("pod"), Some(&"web".to_string()));
        assert_eq!(vars.get("env"), Some(&"staging".to_string()));
    }

    #[test]
    fn test_parse_var_flags_invalid() {
        let result = parse_var_flags(&["noequalssign".to_string()]);
        assert!(result.is_err());
    }
}
