use crate::core::models::{BookLanguage, CommandKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Init(BookLanguage),
    UnsupportedInitLanguage(String),
    Status,
    Authoring(String),
}

pub fn parse_command(text: &str, mentions_bot: bool, bot_name: &str) -> Option<ParsedCommand> {
    if !mentions_bot {
        return None;
    }
    let trimmed = text.trim();
    let normalized = trimmed
        .strip_prefix(&format!("@{bot_name}"))
        .or_else(|| trimmed.strip_prefix("/bookbot"))
        .or_else(|| trimmed.strip_prefix(&format!("/{bot_name}")))
        .map(str::trim)
        .unwrap_or(trimmed);
    let mut parts = normalized.split_whitespace();
    match parts.next() {
        Some("init") => {
            let language = parts.next();
            if parts.next().is_some() {
                return Some(ParsedCommand::UnsupportedInitLanguage(
                    normalized.trim_start_matches("init").trim().to_string(),
                ));
            }
            match language {
                None => Some(ParsedCommand::Init(BookLanguage::English)),
                Some(value) => BookLanguage::parse(value)
                    .map(ParsedCommand::Init)
                    .or_else(|| Some(ParsedCommand::UnsupportedInitLanguage(value.to_string()))),
            }
        }
        Some("status") if parts.next().is_none() => Some(ParsedCommand::Status),
        Some(_) if !normalized.is_empty() => Some(ParsedCommand::Authoring(normalized.to_string())),
        _ => None,
    }
}

impl ParsedCommand {
    pub fn kind(&self) -> CommandKind {
        match self {
            ParsedCommand::Init(_) | ParsedCommand::UnsupportedInitLanguage(_) => CommandKind::Init,
            ParsedCommand::Status => CommandKind::Status,
            ParsedCommand::Authoring(_) => CommandKind::Authoring,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_control_and_authoring_commands() {
        assert_eq!(
            parse_command("/bookbot init", true, "bookbot"),
            Some(ParsedCommand::Init(BookLanguage::English))
        );
        assert_eq!(
            parse_command("/bookbot init en", true, "bookbot"),
            Some(ParsedCommand::Init(BookLanguage::English))
        );
        assert_eq!(
            parse_command("/bookbot init ru", true, "bookbot"),
            Some(ParsedCommand::Init(BookLanguage::Russian))
        );
        assert_eq!(
            parse_command("/bookbot init russian", true, "bookbot"),
            Some(ParsedCommand::Init(BookLanguage::Russian))
        );
        assert_eq!(
            parse_command("/bookbot init deutsch", true, "bookbot"),
            Some(ParsedCommand::UnsupportedInitLanguage(
                "deutsch".to_string()
            ))
        );
        assert_eq!(
            parse_command("/writerbot status", true, "writerbot"),
            Some(ParsedCommand::Status)
        );
        assert_eq!(
            parse_command("@bookbot write a chapter", true, "bookbot"),
            Some(ParsedCommand::Authoring("write a chapter".to_string()))
        );
        assert_eq!(
            parse_command("write a chapter", true, "bookbot"),
            Some(ParsedCommand::Authoring("write a chapter".to_string()))
        );
        assert_eq!(parse_command("hello team", false, "bookbot"), None);
    }
}
