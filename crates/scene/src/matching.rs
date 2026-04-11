use hypreact_core::snapshot::WindowSnapshot;
use hypreact_core::types::WindowShell;
use hypreact_core::{MatchClause, MatchKey, WindowMatch};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MatchParseError {
    #[error("match expression cannot be empty")]
    Empty,
    #[error("unsupported match key `{key}`")]
    UnsupportedKey { key: String },
    #[error("expected `=` after key `{key}`")]
    ExpectedEquals { key: String },
    #[error("expected quoted value for key `{key}`")]
    ExpectedQuotedValue { key: String },
    #[error("unterminated quoted value for key `{key}`")]
    UnterminatedValue { key: String },
    #[error("unexpected trailing content `{content}`")]
    TrailingContent { content: String },
}

pub fn parse_window_match(input: &str) -> Result<WindowMatch, MatchParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(MatchParseError::Empty);
    }

    let mut clauses = Vec::new();
    let mut token = String::new();
    let mut in_quotes = false;
    let mut escaped = false;

    for ch in trimmed.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_quotes => {
                token.push(ch);
                escaped = true;
            }
            '"' => {
                token.push(ch);
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if token.is_empty() {
                    continue;
                }

                let clause = parse_clause(&token)?;
                clauses.push(clause);
                token.clear();
            }
            _ => token.push(ch),
        }
    }

    if !token.is_empty() {
        let clause = parse_clause(&token)?;
        clauses.push(clause);
    }

    Ok(WindowMatch { clauses })
}

pub fn matches_window(window_match: &WindowMatch, window: &WindowSnapshot) -> bool {
    window_match
        .clauses
        .iter()
        .all(|clause| clause_matches(clause, window))
}

fn clause_matches(clause: &MatchClause, window: &WindowSnapshot) -> bool {
    match clause.key {
        MatchKey::AppId => window.app_id.as_deref() == Some(clause.value.as_str()),
        MatchKey::Title => window.title.as_deref() == Some(clause.value.as_str()),
        MatchKey::Class => window.class.as_deref() == Some(clause.value.as_str()),
        MatchKey::Instance => window.instance.as_deref() == Some(clause.value.as_str()),
        MatchKey::Role => window.role.as_deref() == Some(clause.value.as_str()),
        MatchKey::Shell => shell_name(window.shell) == clause.value,
        MatchKey::WindowType => window.window_type.as_deref() == Some(clause.value.as_str()),
    }
}

fn shell_name(shell: WindowShell) -> &'static str {
    match shell {
        WindowShell::Wayland => "wayland",
        WindowShell::Xwayland => "xwayland",
    }
}

fn parse_clause(token: &str) -> Result<MatchClause, MatchParseError> {
    let Some((raw_key, raw_value)) = token.split_once('=') else {
        return Err(MatchParseError::ExpectedEquals {
            key: token.to_owned(),
        });
    };

    let key = parse_key(raw_key)?;
    let Some(raw_value) = raw_value.strip_prefix('"') else {
        return Err(MatchParseError::ExpectedQuotedValue {
            key: raw_key.to_owned(),
        });
    };
    let Some(raw_value) = raw_value.strip_suffix('"') else {
        return Err(MatchParseError::UnterminatedValue {
            key: raw_key.to_owned(),
        });
    };

    if raw_value.contains('"') {
        return Err(MatchParseError::TrailingContent {
            content: token.to_owned(),
        });
    }

    Ok(MatchClause {
        key,
        value: raw_value.replace("\\\"", "\"").replace("\\\\", "\\"),
    })
}

fn parse_key(input: &str) -> Result<MatchKey, MatchParseError> {
    match input {
        "app_id" => Ok(MatchKey::AppId),
        "title" => Ok(MatchKey::Title),
        "class" => Ok(MatchKey::Class),
        "instance" => Ok(MatchKey::Instance),
        "role" => Ok(MatchKey::Role),
        "shell" => Ok(MatchKey::Shell),
        "window_type" => Ok(MatchKey::WindowType),
        other => Err(MatchParseError::UnsupportedKey {
            key: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use hypreact_core::{OutputId, WindowId, WorkspaceId};

    use super::*;

    fn test_window(id: &str) -> WindowSnapshot {
        WindowSnapshot {
            id: WindowId::from(id),
            shell: WindowShell::Wayland,
            app_id: Some("firefox".into()),
            title: Some("Mozilla Firefox".into()),
            class: Some("Navigator".into()),
            instance: Some("navigator".into()),
            role: Some("browser".into()),
            window_type: Some("normal".into()),
            mapped: true,
            mode: hypreact_core::types::WindowMode::Tiled,
            focused: false,
            urgent: false,
            closing: false,
            output_id: Some(OutputId::from("HDMI-A-1")),
            workspace_id: Some(WorkspaceId::from("1")),
            workspaces: vec!["1".into()],
        }
    }

    #[test]
    fn parses_multiple_and_clauses() {
        let parsed = parse_window_match("app_id=\"firefox\" title=\"Mozilla Firefox\"").unwrap();

        assert_eq!(
            parsed,
            WindowMatch {
                clauses: vec![
                    MatchClause {
                        key: MatchKey::AppId,
                        value: "firefox".into()
                    },
                    MatchClause {
                        key: MatchKey::Title,
                        value: "Mozilla Firefox".into()
                    },
                ],
            }
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = parse_window_match("pid=\"42\"").unwrap_err();

        assert_eq!(error, MatchParseError::UnsupportedKey { key: "pid".into() });
    }

    #[test]
    fn rejects_unquoted_values() {
        let error = parse_window_match("app_id=firefox").unwrap_err();

        assert_eq!(
            error,
            MatchParseError::ExpectedQuotedValue {
                key: "app_id".into()
            }
        );
    }

    #[test]
    fn rejects_empty_input() {
        let error = parse_window_match("   ").unwrap_err();

        assert_eq!(error, MatchParseError::Empty);
    }

    #[test]
    fn matches_all_clauses_against_window_snapshot() {
        let window_match = WindowMatch {
            clauses: vec![
                MatchClause {
                    key: MatchKey::AppId,
                    value: "firefox".into(),
                },
                MatchClause {
                    key: MatchKey::Shell,
                    value: "wayland".into(),
                },
            ],
        };

        assert!(matches_window(&window_match, &test_window("win-1")));
    }

    #[test]
    fn mismatched_clause_rejects_window_snapshot() {
        let window_match = WindowMatch {
            clauses: vec![MatchClause {
                key: MatchKey::Title,
                value: "Alacritty".into(),
            }],
        };

        assert!(!matches_window(&window_match, &test_window("win-1")));
    }
}
