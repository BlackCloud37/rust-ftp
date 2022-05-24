use std::{net::{TcpListener, TcpStream}, thread, io::{Write, BufReader, BufRead}};


fn client_handler(mut control_stream: TcpStream) {
    let mut line = String::new();
    let mut control_reader = BufReader::new(control_stream.try_clone().unwrap());
    while let Ok(_) = control_reader.read_line(&mut line) {
        control_stream.write(format!("Your command is {}", line).as_bytes()).unwrap();
        line.clear();
    }
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8000").unwrap();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    client_handler(stream);
                });
            },
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}
