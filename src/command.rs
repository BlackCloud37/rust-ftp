#![allow(unused_variables)]
#![allow(non_snake_case)]

use std::str::FromStr;
use strum_macros::EnumString;

macro_rules! commands {
    ($($cmd: ident), *) => {

        #[derive(EnumString, Debug)]
        #[strum(ascii_case_insensitive)]
        pub enum Command {
            $(
                $cmd(Vec<String>),
            )*
        }

        impl Command {
            /// Parse string to command,
            /// All arguments will be collected as Strings without checking their validity since
            ///     exec should do it
            /// return `None` if command type is unexist
            pub fn parse<S: AsRef<str>>(s: S) -> Option<Self> {
                let tokens = s.as_ref().split_ascii_whitespace().collect::<Vec<_>>();
                if tokens.is_empty() {
                    return None;
                }

                let parse_result = Command::from_str(tokens[0]);
                match parse_result {
                    Ok(command) => {
                        let args = tokens[1..].iter().map(|s| s.to_string()).collect();
                        match command {
                            $($cmd => Some(Self::$cmd(args)),)*
                        }
                    },
                    _ => None
                }
            }

            #[allow(dead_code)]
            pub fn get_args(&self) -> &Vec<String> {
                match self {
                    $(Self::$cmd(v) => &v,)*
                }
            }
        }
    };
}

commands!(Quit);

#[cfg(test)]
mod command_test {
    use super::*;

    #[test]
    fn test_parse_valid() {
        let quit = Command::parse("QUIT\r\n").unwrap();
        assert!(matches!(quit, Command::Quit(_)));
        assert!(quit.get_args().is_empty());

        let case_insensitive_quit = Command::parse("qUiT\r\n").unwrap();
        assert!(matches!(case_insensitive_quit, Command::Quit(_)));
        assert!(case_insensitive_quit.get_args().is_empty());

        let quit_with_arguments = Command::parse("quit a b c 1 2 3\r\n").unwrap();
        assert!(matches!(quit_with_arguments, Command::Quit(_)));
        assert_eq!(quit_with_arguments.get_args().len(), 6);
        for (i, ch) in ["a", "b", "c", "1", "2", "3"].iter().enumerate() {
            assert_eq!(quit_with_arguments.get_args()[i], *ch);
        }
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Command::parse("\r\n").is_none());
        assert!(Command::parse("NONE arg1 arg2 arg3\r\n").is_none());
    }
}
