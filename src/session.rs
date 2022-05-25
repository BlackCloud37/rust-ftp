use std::{net::TcpStream, io::{BufReader, BufWriter, BufRead, Write}};
use anyhow::{Result, anyhow, Ok};

use crate::{command::Command, response, response::ResponseMessage};

/// Session with a client
pub struct Session {
    cmd_reader: BufReader<TcpStream>,
    cmd_writer: BufWriter<TcpStream>,
}

impl Session {
    pub fn new(cmd_stream: TcpStream) -> Result<Self> {
        let cmd_reader = BufReader::new(cmd_stream.try_clone()?);
        let cmd_writer = BufWriter::new(cmd_stream.try_clone()?);
        Ok(Session { cmd_reader, cmd_writer })
    }
    
    /// receive one line message and parse it to command
    /// returns err when failed to get message, thus the conn should be closed
    /// returns ok but the inner value may be none if parse failed
    fn get_cmd(&mut self) -> Result<Option<Command>> {
        let line = self.get_msg()?;
        Ok(Command::parse(&line))
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

    /// send one response to client
    fn send_resp<T>(&mut self, resp: T) -> Result<()>
        where 
            T: ResponseMessage {
        let code = resp.code();
        let message = resp.message();
        self.send_msg(&format!("{code:} {message:}\r\n"))
    }

    /// send one line message to client
    fn send_msg(&mut self, msg: &str) -> Result<()> {
        self.cmd_writer.write_all(msg.as_bytes())?;
        self.cmd_writer.flush()?;
        Ok(())
    }

    /// handle client with a infinite loop, read client's command and exec it 
    pub fn run(&mut self) -> Result<()> {
        self.send_resp(response::Greeting220)?;

        loop {
            let cmd = self.get_cmd()?;
            if let Some(cmd) = cmd {
                self.handle_cmd(cmd)?;
            } else {
                // parse failed
                self.send_resp(response::SyntaxErr500)?;
            }
        }
    }

    /// if err returned, the conn will be shutdown
    fn handle_cmd(&mut self, cmd: Command) -> Result<()> {
        use crate::command::CommandT::*;
        match cmd.cmd_type {
            QUIT => self.handle_quit(cmd.args)?,
        }
        Ok(())
    }

    pub fn handle_quit(&mut self, _args: Vec<String>) -> Result<()> {
        Err(anyhow!("quit"))
    }
}
