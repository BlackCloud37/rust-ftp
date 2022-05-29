//! # response
//! This module contains
//! 1. A trait `ResponseMessage` that describes messages that server reply to client
//! 2. All message structs that impl `ResponseMessage`

use std::fmt::Display;

/// all response has a response code and a message
pub trait ResponseMessage: Sized + Display {
    fn code(&self) -> u16;
    fn message(&self) -> &str;
}

macro_rules! impl_display {
    ($($structname: ty), *) => {
        $(
            impl Display for $structname {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{} {}\r\n", self.code(), self.message())
                }
            }
        )*
    };
}

macro_rules! response {
    ($structname: ident, $code: literal) => {
        pub struct $structname(String);
        impl_display!($structname);
        impl ResponseMessage for $structname {
            fn code(&self) -> u16 {
                $code
            }
            fn message(&self) -> &str {
                &self.0
            }
        }

        impl $structname {
            /// Create response with custom body
            #[allow(dead_code)]
            pub fn new<S: AsRef<str>>(s: S) -> Self {
                Self(s.as_ref().to_string())
            }
        }
    };
    ($structname: ident, $code: literal, $default_message: literal) => {
        pub struct $structname(Option<String>);
        impl_display!($structname);
        impl ResponseMessage for $structname {
            fn code(&self) -> u16 {
                $code
            }
            fn message(&self) -> &str {
                if let Some(s) = &self.0 {
                    return s;
                }
                $default_message
            }
        }

        impl $structname {
            /// Create response with custom body
            #[allow(dead_code)]
            pub fn new<S: AsRef<str>>(s: S) -> Self {
                Self(Some(s.as_ref().to_string()))
            }
        }

        impl Default for $structname {
            fn default() -> Self {
                Self(None)
            }
        }
    };
}

response!(DataTransferStarts150, 150, "150 Here comes the data.");
response!(Greeting220, 220, "Welcome to the rust FTP Server.");
response!(Goodbye221, 221, "Goodbye.");
response!(DataTransferFinished226, 226, "Data transfer finished.");
response!(PasvMode227, 227);
response!(LoginSuccess230, 230, "Login successful.");

response!(NeedPassword331, 331, "Please specify the password.");

response!(ServiceNotAvalible421, 421, "Service not available, closing control connection.");
response!(NoModeSpecified425, 425, "Use PASV first.");

response!(SyntaxErr500, 500, "Command not executed: syntax error.");
response!(InvalidParameter501, 501, "Invalid parameters.");
response!(NotImplementedCommand502, 502, "Command not implemented.");
response!(WrongCmdSequence503, 503, "Wrong command sequence.");
response!(NotLoggedin530, 530, "Please login with USER and PASS.");
response!(UnknownRespWithoutDefaultMessage999, 999);

#[cfg(test)]
mod response_test {
    use super::*;

    fn assert_response_equal_str<T: ResponseMessage>(resp: T, s: &str) {
        assert_eq!(resp.to_string(), format!("{s:}\r\n"));
    }

    #[test]
    fn test_resp_default_message() {
        assert_response_equal_str(Greeting220::default(), "220 Welcome to the rust FTP Server.");
        assert_response_equal_str(
            SyntaxErr500::default(),
            "500 Command not executed: syntax error.",
        );
    }

    #[test]
    fn test_resp_custom_message() {
        assert_response_equal_str(
            UnknownRespWithoutDefaultMessage999::new("unknown"),
            "999 unknown",
        );
    }
}
