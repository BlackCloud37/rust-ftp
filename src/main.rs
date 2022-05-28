mod command;
mod response;
mod session;
use std::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    thread,
};

use anyhow::Result;
use env_logger::Env;
use log::{debug, error, info};
use session::Session;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let addr = "0.0.0.0:8080";
    info!("Starting server at {addr:}");
    serve(addr);
}

fn serve<A: ToSocketAddrs>(addr: A) {
    let listener = TcpListener::bind(addr).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                serve_one_client(stream);
            }
            Err(e) => {
                error!("failed accepting client's connection: {e:}");
            }
        }
    }
}

/// handle client with a infinite loop, read client's command and exec it
fn serve_one_client(stream: TcpStream) {
    let client_addr = stream
        .peer_addr()
        .map_or("unknown".to_string(), |v| v.to_string());

    thread::spawn(move || {
        if let Ok(mut session) = Session::new(stream) {
            let mut run = || -> Result<()> {
                info!("Session with {client_addr:} starts");
                session.send_msg_check_crlf(response::Greeting220::default())?;

                loop {
                    let cmd = session.get_cmd()?;
                    debug!("Parse result: {cmd:?}");
                    if let Some(cmd) = cmd {
                        let resp = session.exec_cmd(cmd)?;
                        session.send_msg_check_crlf(resp)?;
                    } else {
                        // parse failed
                        session.send_msg_check_crlf(response::SyntaxErr500::default())?;
                    }
                }
            };
            if let Err(e) = run() {
                info!("Session with {client_addr:} closed: {e:}");
            }
        } else {
            error!("Error creating session with {client_addr:}");
        }
    });
}

#[cfg(test)]
pub mod integration_test {
    use std::{
        io::{BufRead, BufReader, BufWriter, Write},
        net::TcpStream,
        sync::Once,
        thread::{self, sleep},
        time::Duration,
    };

    use anyhow::{anyhow, Result};
    use log::info;

    use crate::{response::*, serve};

    pub struct TestClient {
        pub(crate) cmd_reader: BufReader<TcpStream>,
        pub(crate) cmd_writer: BufWriter<TcpStream>,
    }

    impl TestClient {
        /// receive one line message from server and trim it
        pub fn get_msg_trimed(&mut self) -> Result<String> {
            let mut line = String::new();
            let bytes = self.cmd_reader.read_line(&mut line).unwrap();
            if bytes == 0 {
                return Err(anyhow!(""));
            }
            Ok(line.trim().to_string())
        }

        pub fn get_msg_code(&mut self) -> Result<u16> {
            let msg = self.get_msg_trimed()?;
            Ok(msg.split_ascii_whitespace().next().unwrap().parse().unwrap())
        }

        /// send one line message to server(with appended \r\n)
        pub fn send_msg_add_crlf(&mut self, msg: &str) -> Result<()> {
            self.cmd_writer
                .write_all(format!("{msg:}\r\n").as_bytes())?;
            self.cmd_writer.flush()?;
            Ok(())
        }
    }

    mod setup {
        use super::*;

        static INIT: Once = Once::new();
        fn setup_once() {
            INIT.call_once(|| {
                init_logger();
                setup_server();
            })
        }

        fn init_logger() {
            let _ = env_logger::builder().is_test(true).try_init();
        }

        fn setup_server() {
            let _server = thread::spawn(move || {
                serve("0.0.0.0:8080");
            });
            // wait server to start
            sleep(Duration::from_micros(100));
            info!("server is up");
        }

        /// returns reader/writer of control conn
        pub fn setup_client() -> TestClient {
            setup_once();
            let client = TcpStream::connect("127.0.0.1:8080").unwrap();
            let cmd_reader = BufReader::new(client.try_clone().unwrap());
            let cmd_writer = BufWriter::new(client.try_clone().unwrap());
            info!("client is up");
            TestClient {
                cmd_reader,
                cmd_writer,
            }
        }
    }

    pub mod utils {
        pub fn assert_string_trim_eq<LS: AsRef<str>, RS: AsRef<str>>(lhs: LS, rhs: RS) {
            assert_eq!(lhs.as_ref().trim(), rhs.as_ref().trim());
        }
    }

    use setup::*;
    use utils::*;

    #[test]
    fn test_hello() {
        let mut client = setup_client();

        assert_string_trim_eq(
            client.get_msg_trimed().unwrap(),
            Greeting220::default().to_string(),
        );
    }

    #[test]
    fn test_quit() {
        let mut client = setup_client();

        client.get_msg_trimed().unwrap(); // ignore hello
        client.send_msg_add_crlf("QUIT").unwrap(); // quit
        assert_eq!(client.get_msg_code().unwrap(), 221);
        assert!(client.get_msg_trimed().is_err()); // conn should close
    }

    #[test]
    fn test_login_success() {
        let mut client = setup_client();

        client.get_msg_trimed().unwrap();

        client.send_msg_add_crlf("USER anonymous").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 331);

        client.send_msg_add_crlf("PASS anonymous").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 230);
    }

    #[test]
    fn test_login_fail() {
        let mut client = setup_client();

        client.get_msg_trimed().unwrap();

        client.send_msg_add_crlf("USER anonymous").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 331);

        client.send_msg_add_crlf("PASS wrong").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 530);

        client.send_msg_add_crlf("PASS anonymous").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 503);
    }
}
