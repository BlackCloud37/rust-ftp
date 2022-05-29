use crate::{
    command::Command,
    response::{self},
    LISTENING_HOST
};
use anyhow::{anyhow, Result};
use log::{error, debug};
use paste::paste;
use std::{
    fmt::Display,
    io::{BufRead, BufReader, BufWriter, Write},
    net::{TcpListener, TcpStream},
};

const FAKE_USER: &str = "anonymous";
const FAKE_PASS: &str = "anonymous";

fn fake_user_valid(username: &str, password: &str) -> bool {
    username == FAKE_USER && password == FAKE_PASS
}

fn get_local_hostname<'a>() -> &'a str {
    "127.0.0.1"
}

/// from h1.h2.h3.h4 to h1,h2,h3,h4
fn hostname_to_comma_hostname(hostname: &str) -> String {
    return hostname.split('.').collect::<Vec<_>>().join(",");
}

#[derive(PartialEq, Debug)]
enum LoginStatus {
    Unloggedin,
    Username(String),
    Loggedin(String),
}

#[derive(Debug)]
enum TransferMode {
    NotSpecified,
    Pasv(u16, TcpListener),
}

/// Session with a client
pub struct Session {
    cmd_reader: BufReader<TcpStream>,
    cmd_writer: BufWriter<TcpStream>,
    login_status: LoginStatus,
    transfer_mode: TransferMode,
}

macro_rules! check_permission_or_return {
    ($self: ident) => {
        match $self.login_status {
            LoginStatus::Username(_) | LoginStatus::Unloggedin => {
                debug!("User not logged in.");
                return Ok(response::NotLoggedin530::default().to_string());
            },
            _ => {}
        };
    };
}

impl Session {
    pub fn new(cmd_stream: TcpStream) -> Result<Self> {
        let cmd_reader = BufReader::new(cmd_stream.try_clone()?);
        let cmd_writer = BufWriter::new(cmd_stream.try_clone()?);
        Ok(Session {
            cmd_reader,
            cmd_writer,
            login_status: LoginStatus::Unloggedin,
            transfer_mode: TransferMode::NotSpecified,
        })
    }

    /// receive one line message and parse it to command
    /// returns err when failed to get message, thus the conn should be closed
    /// returns ok but the inner value may be none if parse failed
    pub fn get_cmd(&mut self) -> Result<Result<Command>> {
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

    fn exec_quit(&mut self, _args: Vec<String>) -> Result<String> {
        self.send_msg_check_crlf(response::Goodbye221::default().to_string())?;
        Err(anyhow!("quit"))
    }

    fn exec_user(&mut self, args: Vec<String>) -> Result<String> {
        let username = &args[0];
        Ok(match self.login_status {
            LoginStatus::Loggedin(_) => {
                response::NotLoggedin530::new("Can't change to another user.").to_string()
            }
            LoginStatus::Unloggedin | LoginStatus::Username(_) => {
                self.login_status = LoginStatus::Username(username.into());
                response::NeedPassword331::default().to_string()
            }
        })
    }

    fn exec_pass(&mut self, args: Vec<String>) -> Result<String> {
        let passwd = &args[0];
        Ok(match &self.login_status {
            LoginStatus::Unloggedin => {
                response::WrongCmdSequence503::new("Login with USER first.").to_string()
            }
            LoginStatus::Loggedin(_) => {
                response::LoginSuccess230::new("Already logged in.").to_string()
            }
            LoginStatus::Username(username) => {
                if fake_user_valid(username, passwd) {
                    self.login_status = LoginStatus::Loggedin(username.into());
                    response::LoginSuccess230::default().to_string()
                } else {
                    self.login_status = LoginStatus::Unloggedin;
                    response::NotLoggedin530::new("Login incorrect.").to_string()
                }
            }
        })
    }

    fn exec_pasv(&mut self, _args: Vec<String>) -> Result<String> {
        check_permission_or_return!(self);
 
        // Does nothing when is in pasv mode already
        if let Some(port) = portpicker::pick_unused_port() {
            if let Ok(listener) = TcpListener::bind(format!("{LISTENING_HOST:}:{port:}")) {
                debug!("Entering pasv mode, listening client on {port:}");
                self.transfer_mode = TransferMode::Pasv(port, listener);

                let (p1, p2) = (port / 256, port % 256);
                let comma_hostname = hostname_to_comma_hostname(get_local_hostname());
                return Ok(response::PasvMode227::new(format!("({comma_hostname:},{p1:},{p2:})")).to_string());    
            }
        }
        error!("No avalible port for pasv or cannot establish listener.");
        Err(anyhow!(response::ServiceNotAvalible421::default().to_string()))
    }

    /// decorate the data_transfer_logic with data conn management logic, so the inner logic don't need to care about it
    fn data_connection_wrapper<F: Fn(&mut TcpStream) -> Result<()>>(&mut self, data_transfer_logic: F) -> Result<String> {
        let transfer_mode = std::mem::replace(&mut self.transfer_mode, TransferMode::NotSpecified);
        match transfer_mode {
            TransferMode::NotSpecified => Ok(response::NoModeSpecified425::default().to_string()),
            TransferMode::Pasv(_, listener) => {
                if let Ok((mut stream, _)) = listener.accept() {
                    self.send_msg_check_crlf(response::DataTransferStarts150::default())?;
                    data_transfer_logic(&mut stream)?;
                    return Ok(response::DataTransferFinished226::default().to_string());
                }
                Err(anyhow!(response::ServiceNotAvalible421::default().to_string()))
            },
        }
    }

    fn exec_list(&mut self, _args: Vec<String>) -> Result<String> {
        check_permission_or_return!(self);
        self.data_connection_wrapper(|stream| -> Result<()> {
            stream.write_all(".\r\n..\r\nthis\r\noutput\r\nis\r\nfake\r\n".as_bytes())?;
            stream.flush()?;
            Ok(())
        })
    }

    fn exec_fakecmdwithtwoarg(&mut self, _args: Vec<String>) -> Result<String> {
        unreachable!()
    }

    fn exec_port(&mut self, _args: Vec<String>) -> Result<String> {
        Ok(response::NotImplementedCommand502::default().to_string())
    }
}



macro_rules! register_command_handlers {
    ($($cmd: ident), *) => {
        impl crate::Session {
            /// Returns Ok(Message) then Message will be send to client
            /// Returns Err(e) then conn will be closed
            pub fn exec_cmd(&mut self, cmd: Command) -> anyhow::Result<String> {
                match cmd {
                    $(
                        // `paste` will concat function names like exec_quit, exec_user and so on
                        //      so that I don't need to write all these match arms by myself
                        Command::$cmd(arg) => paste!{ self.[<exec_ $cmd:lower>](arg) },
                    )*
                }
            }
        }

    }
}

register_command_handlers!(Quit, User, Pass, FakeCmdWithTwoArg, Pasv, Port, List);

#[cfg(test)]
mod session_test {
    use super::*;
    use crate::{integration_test::utils::*, response, integration_test::{USERNAME, PASSWORD}};
    mod setup {
        use super::*;
        use crate::integration_test::TestClient;
        use std::{
            net::TcpListener,
            sync::{Mutex, Once},
            thread, vec,
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

        pub fn setup_client_and_session_unlogged() -> (TestClient, Session) {
            setup_listener();

            let accept_thread = thread::spawn(move || unsafe {
                let listener_guard = LISTENER.as_ref().unwrap().lock().unwrap();
                let conn_thread = thread::spawn(setup_client);
                let (stream, _) = listener_guard.accept().unwrap();
                (conn_thread.join().unwrap(), Session::new(stream).unwrap())
            });
            accept_thread.join().unwrap()
        }
        /// create a TestClient and a Session, the client is connected to the session
        pub fn setup_client_and_session_and_login() -> (TestClient, Session) {
            let (client, mut session) = setup_client_and_session_unlogged();
            session.exec_user(vec![USERNAME.to_string()]).unwrap();
            session.exec_pass(vec![PASSWORD.to_string()]).unwrap();   
            (client, session)
        }
    }

    #[test]
    fn test_create_session() {
        let (_, _) = setup::setup_client_and_session_and_login();
    }

    #[test]
    fn test_send_msg() {
        let (mut client, mut session) = setup::setup_client_and_session_and_login();

        let msg = "message";
        session.send_msg_check_crlf(msg).unwrap();
        assert_string_trim_eq(client.get_msg_trimed().unwrap(), msg);
    }

    #[test]
    fn test_send_resp() {
        let (mut client, mut session) = setup::setup_client_and_session_and_login();

        session
            .send_msg_check_crlf(response::UnknownRespWithoutDefaultMessage999::new(
                "message",
            ))
            .unwrap();
        assert_string_trim_eq(client.get_msg_trimed().unwrap(), "999 message");
    }

    #[test]
    fn test_get_cmd() {
        let (mut client, mut session) = setup::setup_client_and_session_and_login();

        client.send_msg_add_crlf("QUIT arg").unwrap();
        let cmd = session.get_cmd().unwrap();
        assert!(cmd.is_ok());
        assert!(matches!(cmd.unwrap(), Command::Quit(_)));
    }

    #[test]
    fn test_exec_quit() {
        let (_, mut session) = setup::setup_client_and_session_and_login();

        // Quit will return an Err, thus the infinite loop in serve will break and Session will be dropped
        //      thus the stream in Session will be automaticly closed
        assert!(session.exec_cmd(Command::Quit(vec![])).is_err());
    }

    mod test_loggin {
        use super::*;


        #[test]
        fn test_unlogged() {
            let (_, session) = setup::setup_client_and_session_unlogged();

            assert_eq!(session.login_status, LoginStatus::Unloggedin);
        }

        mod test_user {
            use super::*;
            #[test]
            fn test_exec_user_unlogged() {
                let (_, mut session) = setup::setup_client_and_session_unlogged();

                session
                    .exec_cmd(Command::User(vec![USERNAME.into()]))
                    .unwrap();
                assert_eq!(session.login_status, LoginStatus::Username(USERNAME.into()));
            }

            #[test]
            fn test_exec_user_username() {
                let (_, mut session) = setup::setup_client_and_session_and_login();

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
                let (_, mut session) = setup::setup_client_and_session_and_login();

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
                let (_, mut session) = setup::setup_client_and_session_unlogged();

                session
                    .exec_cmd(Command::Pass(vec![PASSWORD.into()]))
                    .unwrap();
                assert_eq!(session.login_status, LoginStatus::Unloggedin);
            }

            #[test]
            fn test_exec_pass_username() {
                let (_, mut session) = setup::setup_client_and_session_and_login();

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
                let (_, mut session) = setup::setup_client_and_session_and_login();

                session.login_status = LoginStatus::Loggedin(USERNAME.into());
                session
                    .exec_cmd(Command::Pass(vec![PASSWORD.into()]))
                    .unwrap();

                // cannot change user
                assert_eq!(session.login_status, LoginStatus::Loggedin(USERNAME.into()));
            }
        }
    }

    mod test_data_transfer {
        use std::{
            thread::{self, sleep},
            time::Duration,
        };

        use super::*;
        mod utils {
            use super::*;
            pub fn data_conn_client_server(session: &Session) -> (TcpStream, TcpStream) {
                match &session.transfer_mode {
                    TransferMode::Pasv(port, listener) => {
                        let port = *port;
                        let try_conn = thread::spawn(move || {
                            let addr = format!("127.0.0.1:{port:}");
                            TcpStream::connect(addr).unwrap()
                        });
                        sleep(Duration::from_secs(1));
                        let (server_conn, _) = listener.accept().unwrap();
                        let client_conn = try_conn.join().unwrap();
                        (client_conn, server_conn)
                    }
                    _ => {
                        panic!()
                    }
                }
            }
    
            pub fn data_conn_client(session: &Session) -> TcpStream {
                match &session.transfer_mode {
                    TransferMode::Pasv(port, _) => {
                        let port = *port;
                        let try_conn = thread::spawn(move || {
                            let addr = format!("127.0.0.1:{port:}");
                            TcpStream::connect(addr).unwrap()
                        });
                        sleep(Duration::from_secs(1));
                        try_conn.join().unwrap()
                    }
                    _ => {
                        panic!()
                    }
                } 
            }
    
        }
        #[test]
        fn test_no_mode() {
            let (_, session) = setup::setup_client_and_session_and_login();

            assert!(matches!(session.transfer_mode, TransferMode::NotSpecified));
        }

        #[test]
        fn test_pasv() {
            let (_, mut session) = setup::setup_client_and_session_and_login();

            assert!(session.exec_cmd(Command::Pasv(vec![])).unwrap().starts_with("227"));
            assert!(matches!(session.transfer_mode, TransferMode::Pasv(_, _)));

            let (mut client_conn, mut server_conn) = utils::data_conn_client_server(&session);
            crate::integration_test::utils::test_connect(&mut server_conn, &mut client_conn)
        }

        #[test]
        fn test_pasv_on_pasv() {
            let (_, mut session) = setup::setup_client_and_session_and_login();

            session.exec_cmd(Command::Pasv(vec![])).unwrap();
            let old_pasv_port = if let TransferMode::Pasv(port, _) = &session.transfer_mode {
                *port
            } else {
                unreachable!()
            };

            session.exec_cmd(Command::Pasv(vec![])).unwrap();
            let new_pasv_port = if let TransferMode::Pasv(port, _) = &session.transfer_mode {
                *port
            } else {
                unreachable!()
            };

            assert_ne!(old_pasv_port, new_pasv_port);
            let (mut client_conn, mut server_conn) = utils::data_conn_client_server(&session);
            crate::integration_test::utils::test_connect(&mut server_conn, &mut client_conn) 
        }

        #[test]
        fn test_list_no_mode() {
            let (_, mut session) = setup::setup_client_and_session_and_login(); 

            assert!(session.exec_cmd(Command::List(vec![".".to_string()])).unwrap().starts_with("425"));
        }

        #[test]
        fn test_list_pasv() {
            let (_, mut session) = setup::setup_client_and_session_and_login(); 

            session.exec_cmd(Command::Pasv(vec![])).unwrap();
            let _ = utils::data_conn_client(&session); // connect to server on pasv port
            assert!(session.exec_cmd(Command::List(vec![".".to_string()])).unwrap().starts_with("226"));
            
            assert!(matches!(session.transfer_mode, TransferMode::NotSpecified));
        }
    }
}
