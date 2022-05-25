mod session;
mod command;
mod response;
use std::{net::{TcpListener}, thread};

use session::Session; 

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8000").unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let client_addr = stream.peer_addr();
                    if let Ok(mut session) = Session::new(stream) {
                        if let Err(e) = session.run() {
                            println!("Error in session with {}: {}", 
                                client_addr.map_or("unknown".to_string(), |v| v.to_string()), 
                                e);
                        }
                    } else {
                        println!("Error creating session");
                    }
                });
            },
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}
