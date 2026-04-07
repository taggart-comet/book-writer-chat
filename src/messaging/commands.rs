use crate::core::models::CommandKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Init,
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
    match normalized {
        "init" => Some(ParsedCommand::Init),
        "status" => Some(ParsedCommand::Status),
        other if !other.is_empty() => Some(ParsedCommand::Authoring(other.to_string())),
        _ => None,
    }
}

impl ParsedCommand {
    pub fn kind(&self) -> CommandKind {
        match self {
            ParsedCommand::Init => CommandKind::Init,
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
            Some(ParsedCommand::Init)
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
