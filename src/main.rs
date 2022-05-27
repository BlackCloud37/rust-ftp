mod command;
mod response;
mod session;
use std::{
    net::{TcpListener, ToSocketAddrs},
    thread,
};

use env_logger::Env;
use log::{error, info};
use session::Session;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let addr = "0.0.0.0:8080";
    info!("Starting server at {addr:}");
    serve(addr);
}

fn serve<A>(addr: A)
where
    A: ToSocketAddrs,
{
    let listener = TcpListener::bind(addr).unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_addr = stream
                    .peer_addr()
                    .map_or("unknown".to_string(), |v| v.to_string());

                thread::spawn(move || {
                    if let Ok(mut session) = Session::new(stream) {
                        info!("Session with {client_addr:} starts");
                        if let Err(e) = session.run() {
                            info!("Session with {client_addr:} closed: {e:}");
                        }
                    } else {
                        error!("Error creating session with {client_addr:}");
                    }
                });
            }
            Err(e) => {
                error!("failed accepting client's connection: {e:}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader, BufWriter, Write},
        net::TcpStream,
        thread::{self, sleep},
        time::Duration,
    };

    use anyhow::{anyhow, Result};
    use log::info;

    use crate::{response::*, serve};

    struct TestClient {
        cmd_reader: BufReader<TcpStream>,
        cmd_writer: BufWriter<TcpStream>,
    }

    impl TestClient {
        /// receive one line message from client
        fn get_msg(&mut self) -> Result<String> {
            let mut line = String::new();
            let bytes = self.cmd_reader.read_line(&mut line).unwrap();
            if bytes == 0 {
                return Err(anyhow!(""));
            }
            Ok(line.trim().to_string())
        }

        /// send one line message to client(with appended \r\n)
        fn send_msg(&mut self, msg: &str) -> Result<()> {
            self.cmd_writer
                .write_all(format!("{msg:}\r\n").as_bytes())?;
            self.cmd_writer.flush()?;
            Ok(())
        }
    }

    fn init_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn setup_server() {
        let _server = thread::spawn(move || {
            serve("0.0.0.0:8080");
        });
        // wait server to start
        sleep(Duration::from_secs(1));
        info!("server is up");
    }

    /// returns reader/writer of control conn
    fn setup_client() -> TestClient {
        init_logger();
        let client = TcpStream::connect("127.0.0.1:8080").unwrap();
        let cmd_reader = BufReader::new(client.try_clone().unwrap());
        let cmd_writer = BufWriter::new(client.try_clone().unwrap());
        info!("client is up");
        TestClient {
            cmd_reader,
            cmd_writer,
        }
    }

    #[test]
    fn client_server() {
        setup_server();
        let mut client = setup_client();
        assert_eq!(client.get_msg().unwrap(), Greeting220.to_string().trim()); // get hello
        client.send_msg("QUIT").unwrap(); // quit
        assert!(client.get_msg().is_err()); // conn should close
    }
}
