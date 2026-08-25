const COMMANDS: &[&str] = &["daemon", "start", "toggle", "show", "hide", "quit", "stop"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Version,
    Help,
    Run(String),
}

/// Parse `omapaste`'s first CLI argument (argv[1]).
pub fn parse_command(arg: Option<&str>) -> Result<Action, String> {
    match arg {
        Some("--version") | Some("-V") => Ok(Action::Version),
        Some("--help") | Some("-h") => Ok(Action::Help),
        Some(cmd) if COMMANDS.contains(&cmd) => Ok(Action::Run(cmd.to_string())),
        Some(other) => Err(format!("unknown command: {other}")),
        None => Ok(Action::Run("toggle".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_toggles() {
        assert_eq!(parse_command(None), Ok(Action::Run("toggle".into())));
    }

    #[test]
    fn known_commands() {
        for cmd in COMMANDS {
            assert_eq!(parse_command(Some(cmd)), Ok(Action::Run((*cmd).into())));
        }
    }

    #[test]
    fn version_and_help_flags() {
        assert_eq!(parse_command(Some("--version")), Ok(Action::Version));
        assert_eq!(parse_command(Some("-V")), Ok(Action::Version));
        assert_eq!(parse_command(Some("--help")), Ok(Action::Help));
        assert_eq!(parse_command(Some("-h")), Ok(Action::Help));
    }

    #[test]
    fn unknown_command_errors() {
        let err = parse_command(Some("launch")).unwrap_err();
        assert!(err.contains("launch"));
        assert!(parse_command(Some("")).is_err());
        assert!(parse_command(Some("--daemon")).is_err());
    }
}
