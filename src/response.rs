//! # response
//! This module contains
//! 1. A trait `ResponseMessage` that describes messages that server reply to client
//! 2. All message structs that impl `ResponseMessage`





/// all response has a response code and a message
pub trait ResponseMessage: Sized {
    fn code(&self) -> u16;
    fn message(&self) -> &str;
}

pub struct Greeting220;
impl ResponseMessage for Greeting220 {
    fn code(&self) -> u16 {
        220
    }
    fn message(&self) -> &str {
        "Welcome to the rust FTP Server"
    }
}

pub struct SyntaxErr500;
impl ResponseMessage for SyntaxErr500 {
    fn code(&self) -> u16 {
        500
    }
    fn message(&self) -> &str {
        "Command not executed: syntax error"
    }
}