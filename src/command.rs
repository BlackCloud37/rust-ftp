use std::str::FromStr;

use strum_macros::EnumString;

// `#[non_exhaustive]` allows adding variants without breaking users who match the enum
#[non_exhaustive]
#[derive(EnumString)]
#[strum(ascii_case_insensitive)]
pub enum CommandT {    
    QUIT,
}

pub struct Command {
    pub cmd_type: CommandT,
    pub args: Vec<String>
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        let tokens = s.split_ascii_whitespace().collect::<Vec<_>>();
        if tokens.len() == 0 {
            return None;
        }
        match CommandT::from_str(tokens[0]) {
            Ok(cmd) => Some(Self { cmd_type: cmd, args: tokens[1..].into_iter().map(|t| t.to_string()).collect() }),
            Err(_) => None
        }
    }
}