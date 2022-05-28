use crate::{command::Command, response};
use anyhow::{anyhow, Ok, Result};
use log::debug;
use paste::paste;
use std::{
    fmt::Display,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
};

fn fake_user_valid(username: &str, password: &str) -> bool {
    username == "anonymous" && password == "anonymous"
}

#[derive(PartialEq, Debug)]
enum LoginStatus {
    Unloggedin,
    Username(String),
    Loggedin(String),
}

/// Session with a client
pub struct Session {
    cmd_reader: BufReader<TcpStream>,
    cmd_writer: BufWriter<TcpStream>,
    login_status: LoginStatus,
}

impl Session {
    pub fn new(cmd_stream: TcpStream) -> Result<Self> {
        let cmd_reader = BufReader::new(cmd_stream.try_clone()?);
        let cmd_writer = BufWriter::new(cmd_stream.try_clone()?);
        Ok(Session {
            cmd_reader,
            cmd_writer,
            login_status: LoginStatus::Unloggedin,
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

    fn exec_quit(&mut self, _args: Vec<String>) -> Result<()> {
        Err(anyhow!(""))
    }

    fn exec_user(&mut self, args: Vec<String>) -> Result<()> {
        // TODO: dont repeat it
        if args.is_empty() {
            self.send_msg_check_crlf(response::SyntaxErr500::default())?;
            return Ok(());
        }

        let username = &args[0];
        if username.is_empty() {
            self.send_msg_check_crlf(response::SyntaxErr500::default())?;
            return Ok(());
        }

        match self.login_status {
            LoginStatus::Unloggedin | LoginStatus::Username(_) => {
                self.login_status = LoginStatus::Username(username.into());
                self.send_msg_check_crlf(response::NeedPassword331::default())?;
            }
            LoginStatus::Loggedin(_) => self.send_msg_check_crlf(response::NotLoggedin530::new("Can't change to another user."))?,
        }
        Ok(())
    }

    fn exec_pass(&mut self, args: Vec<String>) -> Result<()> {
        // TODO: dont repeat it
        if args.is_empty() {
            self.send_msg_check_crlf(response::SyntaxErr500::default())?;
            return Ok(());
        }

        let passwd = &args[0];
        if passwd.is_empty() {
            self.send_msg_check_crlf(response::SyntaxErr500::default())?;
            return Ok(());
        }

        match &self.login_status {
            LoginStatus::Unloggedin => {
                self.send_msg_check_crlf(response::WrongCmdSequence503::new("Login with USER first."))?
            }
            LoginStatus::Username(username) => {
                if fake_user_valid(username, passwd) {
                    self.login_status =LoginStatus::Loggedin(username.into());
                    self.send_msg_check_crlf(response::LoginSuccess230::default())?;
                } else {
                    self.login_status = LoginStatus::Unloggedin;
                    self.send_msg_check_crlf(response::NotLoggedin530::new("Login incorrect."))?;
                }
            }
            LoginStatus::Loggedin(_) => self.send_msg_check_crlf(response::LoginSuccess230::new("Already logged in."))?,
        }
        Ok(())
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

register_command_handlers!(Quit, User, Pass);

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

    mod test_loggin {
        use super::*;

        static USERNAME: &str = "anonymous";
        static PASSWORD: &str = "anonymous";

        #[test]
        fn test_unlogged() {
            let (_, session) = setup::setup_client_and_session();

            assert_eq!(session.login_status, LoginStatus::Unloggedin);
        }

        #[test]
        fn test_wrong_arguments() {
            let (_, mut session) = setup::setup_client_and_session();

            session.exec_cmd(Command::User(vec![])).unwrap();
            assert_eq!(session.login_status, LoginStatus::Unloggedin);
            session.exec_cmd(Command::Pass(vec![])).unwrap();
            assert_eq!(session.login_status, LoginStatus::Unloggedin);
        }

        mod test_user {
            use super::*;
            #[test]
            fn test_exec_user_unlogged() {
                let (_, mut session) = setup::setup_client_and_session();

                session
                    .exec_cmd(Command::User(vec![USERNAME.into()]))
                    .unwrap();
                assert_eq!(session.login_status, LoginStatus::Username(USERNAME.into()));
            }

            #[test]
            fn test_exec_user_username() {
                let (_, mut session) = setup::setup_client_and_session();

                session.login_status = LoginStatus::Username("oldusername".into());
                session
                    .exec_cmd(Command::User(vec!["newusername".into()]))
                    .unwrap();

                // can change username
                assert_eq!(
                    session.login_status,
                    LoginStatus::Username("newusername".into())
                );
            }

            #[test]
            fn test_exec_user_loggedin() {
                let (_, mut session) = setup::setup_client_and_session();

                session.login_status = LoginStatus::Loggedin("oldusername".into());
                session
                    .exec_cmd(Command::User(vec!["newusername".into()]))
                    .unwrap();

                // cannot change user
                assert_eq!(
                    session.login_status,
                    LoginStatus::Loggedin("oldusername".into())
                );
            }
        }

        mod test_pass {
            use super::*;

            #[test]
            fn test_exec_pass_unlogged() {
                let (_, mut session) = setup::setup_client_and_session();

                session
                    .exec_cmd(Command::Pass(vec![PASSWORD.into()]))
                    .unwrap();
                assert_eq!(session.login_status, LoginStatus::Unloggedin);
            }

            #[test]
            fn test_exec_pass_username() {
                let (_, mut session) = setup::setup_client_and_session();

                session.login_status = LoginStatus::Username(USERNAME.into());
                session
                    .exec_cmd(Command::Pass(vec!["wrongpassword".into()]))
                    .unwrap();
                // status back to Unloggedin
                assert_eq!(session.login_status, LoginStatus::Unloggedin);

                session.login_status = LoginStatus::Username(USERNAME.into());
                // right password
                session
                    .exec_cmd(Command::Pass(vec![PASSWORD.into()]))
                    .unwrap();
                // login success
                assert_eq!(session.login_status, LoginStatus::Loggedin(USERNAME.into()))
            }

            #[test]
            fn test_exec_pass_loggedin() {
                let (_, mut session) = setup::setup_client_and_session();

                session.login_status = LoginStatus::Loggedin(USERNAME.into());
                session
                    .exec_cmd(Command::Pass(vec![PASSWORD.into()]))
                    .unwrap();

                // cannot change user
                assert_eq!(session.login_status, LoginStatus::Loggedin(USERNAME.into()));
            }
        }
    }
}
