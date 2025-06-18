use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};

fn handle_client(mut stream: TcpStream) {
    let mut data = [0 as u8; 32]; // using 50 byte buffer
    // let timeout = time::Duration::from_secs(5);
    // let mut deadline = SystemTime::now() + timeout;
    let mut message = String::new();
    while match stream.read_exact(&mut data) {
        Ok(_) => {
            // if deadline < SystemTime::now() {
            //     println!("Connection timed out.");
            //     false
            // } else {
            stream.write(b"ok").unwrap();
            // deadline = SystemTime::now() + timeout;
            true
            // }
        },
        Err(_) => {
            println!("An error occurred, terminating connection with {}", stream.peer_addr().unwrap());
            stream.shutdown(Shutdown::Both).unwrap();
            false
        }
    } {
        message = message + core::str::from_utf8(&data).unwrap();
        if data.contains(&0u8) {
            println!("{}", message);
            message = String::new();
        }
    }
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:3333").unwrap();
    // accept connections and process them, spawning a new thread for each one
    println!("Server listening on port 3333");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move|| {
                    // connection succeeded
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
                /* connection failed */
            }
        }
    }
    // close the socket server
    drop(listener);
}