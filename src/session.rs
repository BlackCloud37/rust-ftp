use anyhow::{anyhow, Ok, Result};
use log::debug;
use std::{
    fmt::Display,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
};

use crate::command::Command;

/// Session with a client
pub struct Session {
    cmd_reader: BufReader<TcpStream>,
    cmd_writer: BufWriter<TcpStream>,
}

impl Session {
    pub fn new(cmd_stream: TcpStream) -> Result<Self> {
        let cmd_reader = BufReader::new(cmd_stream.try_clone()?);
        let cmd_writer = BufWriter::new(cmd_stream.try_clone()?);
        Ok(Session {
            cmd_reader,
            cmd_writer,
        })
    }

    /// receive one line message and parse it to command
    /// returns err when failed to get message, thus the conn should be closed
    /// returns ok but the inner value may be none if parse failed
    pub fn get_cmd(&mut self) -> Result<Option<Command>> {
        let line = self.get_msg()?;
        let line = line.trim();
        debug!("Recv message: {line:}");
        Ok(Command::parse(line))
    }

    /// receive one line message from client
    fn get_msg(&mut self) -> Result<String> {
        let mut buf = String::new();
        let len = self.cmd_reader.read_line(&mut buf)?;
        if len == 0 {
            return Err(anyhow!("EOF reached, connection closed"));
        }
        Ok(buf)
    }

    /// send one line message to client
    pub fn send_msg<T>(&mut self, msg: T) -> Result<()>
    where
        T: Display,
    {
        let msg = msg.to_string();
        debug!("Send message: {}", msg.trim());
        self.cmd_writer.write_all(msg.as_bytes())?;
        self.cmd_writer.flush()?;
        Ok(())
    }

    /// if err returned, the conn will be shutdown
    pub fn exec_cmd(&mut self, cmd: Command) -> Result<()> {
        use crate::command::CommandT::*;
        match cmd.cmd_type {
            Quit => self.handle_quit(cmd.args)?,
        }
        Ok(())
    }

    fn handle_quit(&mut self, _args: Vec<String>) -> Result<()> {
        Err(anyhow!("quit"))
    }
}

#[cfg(test)]
mod session_test {
    #[test]
    fn it_works() {
        assert_eq!(1 + 1, 2);
    }
}
