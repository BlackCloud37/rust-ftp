use crate::command::Command;
use anyhow::{anyhow, Ok, Result};
use log::debug;
use paste::paste;
use std::{
    fmt::Display,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
};

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
        let line = self.get_msg_not_trimmed()?;
        let line = line.trim();
        debug!("Recv message: {line:}");
        Ok(Command::parse(line))
    }

    /// receive one line message from client
    fn get_msg_not_trimmed(&mut self) -> Result<String> {
        let mut buf = String::new();
        let len = self.cmd_reader.read_line(&mut buf)?;
        if len == 0 {
            return Err(anyhow!("EOF reached, connection closed"));
        }
        Ok(buf)
    }

    /// send one line message to client
    pub fn send_msg_check_crlf<T>(&mut self, msg: T) -> Result<()>
    where
        T: Display,
    {
        let mut msg = msg.to_string();
        if !msg.ends_with("\r\n") {
            msg = format!("{msg:}\r\n");
        }
        debug!("Send message: {}", msg.trim());
        self.cmd_writer.write_all(msg.as_bytes())?;
        self.cmd_writer.flush()?;
        Ok(())
    }

    pub fn exec_quit(&mut self, _args: Vec<String>) -> Result<()> {
        Err(anyhow!(""))
    }
}

macro_rules! register_command_handlers {
    ($($cmd: ident), *) => {
        impl crate::Session {
            pub fn exec_cmd(&mut self, cmd: Command) -> anyhow::Result<()> {
                match cmd {
                    $(
                        Command::$cmd(arg) => paste!{ self.[<exec_ $cmd:lower>](arg) },
                    )*
                }
            }
        }

    }
}

register_command_handlers!(Quit);

#[cfg(test)]
mod session_test {
    use super::*;
    use crate::{integration_test::utils::*, response};
    mod setup {
        use super::*;
        use crate::integration_test::TestClient;
        use std::{
            net::TcpListener,
            sync::{Mutex, Once},
            thread,
        };

        static INIT: Once = Once::new();
        static mut LISTENER: Option<Mutex<TcpListener>> = None;

        // setup a listener and move it into LISTENER
        fn setup_listener() {
            INIT.call_once(|| unsafe {
                let listener = TcpListener::bind("0.0.0.0:12345").unwrap();
                LISTENER = Some(Mutex::new(listener))
            })
        }

        fn setup_client() -> TestClient {
            let client = TcpStream::connect("127.0.0.1:12345").unwrap();
            let cmd_reader = BufReader::new(client.try_clone().unwrap());
            let cmd_writer = BufWriter::new(client.try_clone().unwrap());
            TestClient {
                cmd_reader,
                cmd_writer,
            }
        }

        /// create a TestClient and a Session, the client is connected to the session
        pub fn setup_client_and_session() -> (TestClient, Session) {
            setup_listener();

            let accept_thread = thread::spawn(move || unsafe {
                let listener_guard = LISTENER.as_ref().unwrap().lock().unwrap();
                let conn_thread = thread::spawn(setup_client);
                let (stream, _) = listener_guard.accept().unwrap();
                (conn_thread.join().unwrap(), Session::new(stream).unwrap())
            });
            accept_thread.join().unwrap()
        }
    }

    #[test]
    fn test_create_session() {
        let (_, _) = setup::setup_client_and_session();
    }

    #[test]
    fn test_send_msg() {
        let (mut client, mut session) = setup::setup_client_and_session();

        let msg = "message";
        session.send_msg_check_crlf(msg).unwrap();
        assert_string_trim_eq(client.get_msg_trimed().unwrap(), msg);
    }

    #[test]
    fn test_send_resp() {
        let (mut client, mut session) = setup::setup_client_and_session();

        session
            .send_msg_check_crlf(response::UnknownRespWithoutDefaultMessage999::new(
                "message",
            ))
            .unwrap();
        assert_string_trim_eq(client.get_msg_trimed().unwrap(), "999 message");
    }

    #[test]
    fn test_get_cmd() {
        let (mut client, mut session) = setup::setup_client_and_session();

        client.send_msg_add_crlf("QUIT arg").unwrap();
        let cmd = session.get_cmd().unwrap();
        assert!(cmd.is_some());
        assert!(matches!(cmd.unwrap(), Command::Quit(_)));
    }

    #[test]
    fn test_exec_quit() {
        let (_, mut session) = setup::setup_client_and_session();

        // Quit will return an Err, thus the infinite loop in serve will break and Session will be dropped
        //      thus the stream in Session will be automaticly closed
        assert!(session.exec_cmd(Command::Quit(vec![])).is_err());
    }
}
