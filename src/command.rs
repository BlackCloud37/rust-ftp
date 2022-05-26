use std::str::FromStr;

use strum_macros::EnumString;

// `#[non_exhaustive]` allows adding variants without breaking users who match the enum
#[non_exhaustive]
#[derive(EnumString, PartialEq, Debug)]
#[strum(ascii_case_insensitive)]
pub enum CommandT {
    Quit,
}

#[derive(PartialEq, Debug)]
pub struct Command {
    pub cmd_type: CommandT,
    pub args: Vec<String>,
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        let tokens = s.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            return None;
        }
        match CommandT::from_str(tokens[0]) {
            Ok(cmd) => Some(Self {
                cmd_type: cmd,
                args: tokens[1..].iter().map(|t| t.to_string()).collect(),
            }),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ok() {
        let quit = Command::parse("QUIT").unwrap();
        assert_eq!(quit.cmd_type, CommandT::Quit);
        assert_eq!(quit.args.len(), 0);

        let quit_case_insensitive = Command::parse("qUiT").unwrap();
        assert_eq!(quit_case_insensitive.cmd_type, CommandT::Quit);
        assert_eq!(quit_case_insensitive.args.len(), 0);

        let with_arg = Command::parse("QUIT arg1 arg2 arg3").unwrap();
        assert_eq!(with_arg.cmd_type, CommandT::Quit);
        assert_eq!(with_arg.args[0], "arg1");
        assert_eq!(with_arg.args[1], "arg2");
        assert_eq!(with_arg.args[2], "arg3");
    }

    #[test]
    fn test_parse_unexist() {
        assert_eq!(Command::parse(""), None);
        assert_eq!(Command::parse("NONE arg1 arg2 arg3"), None);
    }
}
