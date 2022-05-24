mod session;
use std::{net::{TcpListener}, thread};

use session::Session; 

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8000").unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let client_addr = stream.peer_addr();
                    let mut session = Session {
                        cmd_stream: stream,
                        mode: None,
                    };
                    if let Err(e) = session.run() {
                        println!("Error in session with {:#?}: {}", client_addr, e);
                    }
                });
            },
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}
