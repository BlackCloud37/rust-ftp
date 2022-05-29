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

static LISTENING_HOST: &str = "0.0.0.0";

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let addr = LISTENING_HOST.to_owned() + ":" + "8080";
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
                    match cmd {
                        Ok(cmd) => {
                            let resp = session.exec_cmd(cmd)?;
                            session.send_msg_check_crlf(resp)?;
                        },
                        Err(e) => {
                            session.send_msg_check_crlf(e.to_string())?;
                        }
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

    pub const USERNAME: &str = "anonymous";
    pub const PASSWORD: &str = "anonymous";

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
        use crate::LISTENING_HOST;
        
        const TEST_PORT: u16 = 8080;

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
                serve(format!("{LISTENING_HOST:}:{TEST_PORT:}"));
            });
            // wait server to start
            sleep(Duration::from_micros(100));
            info!("server is up");
        }

        /// returns reader/writer of control conn
        pub fn setup_client() -> TestClient {
            setup_once();
            let client = TcpStream::connect(format!("127.0.0.1:{TEST_PORT:}")).unwrap();
            let cmd_reader = BufReader::new(client.try_clone().unwrap());
            let cmd_writer = BufWriter::new(client.try_clone().unwrap());
            info!("client is up");
            TestClient {
                cmd_reader,
                cmd_writer,
            }
        }

        pub fn setup_client_login() -> TestClient {
            let mut client = setup_client();

            client.get_msg_trimed().unwrap();
    
            client.send_msg_add_crlf(&format!("USER {USERNAME:}")).unwrap();
            assert_eq!(client.get_msg_code().unwrap(), 331);
    
            client.send_msg_add_crlf(&format!("PASS {PASSWORD:}")).unwrap();
            assert_eq!(client.get_msg_code().unwrap(), 230);

            client
        }
    }

    pub mod utils {
        use std::{net::TcpStream, io::{BufReader, Write, BufRead}};

        pub fn assert_string_trim_eq<LS: AsRef<str>, RS: AsRef<str>>(lhs: LS, rhs: RS) {
            assert_eq!(lhs.as_ref().trim(), rhs.as_ref().trim());
        }

        pub fn parse_pasv_response(s: &str) -> String {
            let mut split = s.split_ascii_whitespace();
            split.next();
            let pasv_part = split.next().unwrap();
            let pasv = &pasv_part[1..pasv_part.len()-1]; // (..)
            let splited_pasv = pasv.split(',').collect::<Vec<_>>();
            println!("{s:} {:?}", splited_pasv);
            let h1 = splited_pasv[0];
            let h2 = splited_pasv[1];
            let h3 = splited_pasv[2];
            let h4 = splited_pasv[3];
            let p1 = splited_pasv[4];
            let p2 = splited_pasv[5];
            let port: u16 = p1.parse::<u16>().unwrap() * 256 + p2.parse::<u16>().unwrap();
            format!(
                "{h1:}.{h2:}.{h3:}.{h4:}:{port:}"
            )
        }

        pub fn data_conn_to_pasv_response(s: &str) -> TcpStream {
            let addr = parse_pasv_response(s);
            println!("{addr:}");
            TcpStream::connect(addr).unwrap()
        }

        pub fn test_connect(stream_a: &mut TcpStream, stream_b: &mut TcpStream) {
            println!("{:?}", stream_a.peer_addr());
            println!("{:?}", stream_b.peer_addr());
            assert_eq!(stream_b.write("hello\r\n".as_bytes()).unwrap(), 7);
            let mut reader = BufReader::new(stream_a);
            let mut recv_buf = String::new();
            let count = reader.read_line(&mut recv_buf).unwrap();
            assert_eq!(count, 7);
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

        client.send_msg_add_crlf(&format!("USER {USERNAME:}")).unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 331);

        client.send_msg_add_crlf(&format!("PASS {PASSWORD:}")).unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 230);
    }

    #[test]
    fn test_login_fail() {
        let mut client = setup_client();

        client.get_msg_trimed().unwrap();

        client.send_msg_add_crlf(&format!("USER {USERNAME:}")).unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 331);

        client.send_msg_add_crlf("PASS wrong").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 530);

        client.send_msg_add_crlf(&format!("PASS {PASSWORD:}")).unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 503);
    }

    #[test]
    fn test_permission() {
        let mut client = setup_client();

        client.get_msg_trimed().unwrap();

        client.send_msg_add_crlf("LIST").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 530);

        client.send_msg_add_crlf("PASV").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 530);
    }

    #[test]
    fn test_list_pasv() {
        let mut client = setup_client_login();

        client.send_msg_add_crlf("LIST").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 425);

        client.send_msg_add_crlf("PASV").unwrap();
        let pasv_resp = client.get_msg_trimed().unwrap();
        assert!(pasv_resp.starts_with("227"));

        let _ = BufReader::new(data_conn_to_pasv_response(&pasv_resp));
        client.send_msg_add_crlf("LIST").unwrap();
        assert_eq!(client.get_msg_code().unwrap(), 150);
        assert_eq!(client.get_msg_code().unwrap(), 226); 
    }
}
