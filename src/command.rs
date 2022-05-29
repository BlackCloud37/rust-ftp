#![allow(unused_variables)]
#![allow(non_snake_case)]

use crate::response;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use strum_macros::EnumString;

macro_rules! commands {
    ($($cmd: ident ($argc: literal)), *) => {

        #[derive(EnumString, Debug)]
        #[strum(ascii_case_insensitive)]
        pub enum Command {
            $(
                $cmd(Vec<String>),
            )*
        }

        impl Command {
            /// Parse string to command,
            /// Returns Ok(Command) when command is valid, all arguments will be collected as Strings' vec
            ///     and the length of vec will be equal with the Command's required argument
            ///     if argument is too many, the parse will still be Ok, but if arguments is too less, it will be Err
            /// Returns `Err(Message)` if command is not valid, Message should be sent to client
            pub fn parse<S: AsRef<str>>(s: S) -> Result<Self> {
                let tokens = s.as_ref().split_ascii_whitespace().collect::<Vec<_>>();
                if tokens.is_empty() {
                   return Err(anyhow!(response::SyntaxErr500::default().to_string()));
                }

                let parse_result = Command::from_str(tokens[0]);
                match parse_result {
                    Ok(command) => {
                        match command {
                            $(
                                Self::$cmd(_) => {
                                    // TODO: deal with escape char or space in arguments
                                    if $argc == 0 {
                                        let arg = tokens.into_iter().skip(1).map(|s| s.to_string()).collect::<Vec<_>>().join(" ");
                                        return if arg.is_empty() {
                                            Ok(Self::$cmd(vec![]))
                                        } else {
                                            Ok(Self::$cmd(vec![arg]))
                                        }
                                    }
                                    let mut tokens = tokens.into_iter().skip(1).map(|s| s.to_string());
                                    let mut args = Vec::with_capacity($argc);
                                    loop {
                                        if args.len() + 1 == $argc {
                                            let last_argument = tokens.collect::<Vec<String>>().join(" ");
                                            if !last_argument.is_empty() {
                                                args.push(last_argument);
                                            }
                                            break;
                                        }
                                        if let Some(token) = tokens.next() {
                                            args.push(token);
                                        } else {
                                            break;
                                        }
                                    };

                                    #[allow(unused_comparisons)]
                                    if args.len() < $argc {
                                        return Err(anyhow!(response::InvalidParameter501::new("Invalid number of arguments.").to_string()));
                                    }
                                    return Ok(Self::$cmd(args));
                                }
                            )*
                        }
                    },
                    _ => Err(anyhow!(response::SyntaxErr500::new("Command not understood.").to_string())),
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

commands!(Quit(0), User(1), Pass(1), FakeCmdWithTwoArg(2), Pasv(0), Port(1), List(0));

#[cfg(test)]
mod command_test {
    use super::*;

    #[test]
    fn test_parse_case_insensitive() {
        let quit = Command::parse("QUIT\r\n").unwrap();
        assert!(matches!(quit, Command::Quit(_)));
        assert!(quit.get_args().is_empty());

        let case_insensitive_quit = Command::parse("qUiT\r\n").unwrap();
        assert!(matches!(case_insensitive_quit, Command::Quit(_)));
        assert!(case_insensitive_quit.get_args().is_empty());
    }

    #[test]
    fn test_parse_right_arguments() {
        let user_with_arguments = Command::parse("user username\r\n").unwrap();
        assert!(matches!(user_with_arguments, Command::User(_)));
        assert_eq!(user_with_arguments.get_args().len(), 1);
        assert_eq!(user_with_arguments.get_args()[0], "username");
    }

    #[test]
    fn test_parse_too_many_arguments() {
        let user_with_arguments = Command::parse("user username1 username2\r\n").unwrap();
        assert!(matches!(user_with_arguments, Command::User(_)));
        // arguments will be concat if they are too many
        assert_eq!(user_with_arguments.get_args().len(), 1);
        assert_eq!(user_with_arguments.get_args()[0], "username1 username2");

        let pass_with_arguments = Command::parse("pass a b c d e f g\r\n").unwrap();
        assert!(matches!(pass_with_arguments, Command::Pass(_)));
        // arguments will be concat if they are too many
        assert_eq!(pass_with_arguments.get_args().len(), 1);
        assert_eq!(pass_with_arguments.get_args()[0], "a b c d e f g");

        let fake_with_arguments = Command::parse("FakeCmdWithTwoArg a b c d\r\n").unwrap();
        assert!(matches!(fake_with_arguments, Command::FakeCmdWithTwoArg(_)));
        assert_eq!(fake_with_arguments.get_args().len(), 2);
        assert_eq!(fake_with_arguments.get_args()[0], "a");
        assert_eq!(fake_with_arguments.get_args()[1], "b c d");
    }

    #[test]
    fn test_parse_too_less_arguments() {
        let user_with_no_arguments = Command::parse("user\r\n");
        // is error and response should be 501
        let err = user_with_no_arguments.err().unwrap();
        assert!(err.to_string().starts_with("501"));

        let fake_cmd = Command::parse("FakeCmdWithTwoArg arg\r\n");
        let err = fake_cmd.err().unwrap();
        assert!(err.to_string().starts_with("501"));
    }

    #[test]
    fn test_parse_type() {
        let quit = Command::parse("QUIT\r\n").unwrap();
        assert!(matches!(quit, Command::Quit(_)));

        let user = Command::parse("USER username\r\n").unwrap();
        assert!(matches!(user, Command::User(_)));

        let pass = Command::parse("PASS password\r\n").unwrap();
        assert!(matches!(pass, Command::Pass(_)));
    }

    #[test]
    fn test_parse_syntax_error_or_unexist() {
        let empty_err = Command::parse("\r\n").err().unwrap();
        assert!(empty_err.to_string().starts_with("500"));
        let none_err = Command::parse("NONE arg1 arg2 arg3\r\n").err().unwrap();
        assert!(none_err.to_string().starts_with("500"));
    }
}
