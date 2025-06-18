use std::io::{Error, Read, Write};
use std::net::TcpStream;
use std::str::from_utf8;

pub struct Voice {
    stream: TcpStream,
}

impl Voice {
    pub fn new() -> Result<Self, Error> {
        match TcpStream::connect("localhost:3333") {
            Ok(stream) => {
                println!("Successfully connected to server in port 3333");
                Ok(Voice { stream })
            }
            Err(e) => Err(e),
        }
    }

    fn send_chunk(&mut self, chunk: &[u8]) -> bool {
        if chunk.len() < 32 {
            let mut send_buf = [0 as u8; 32];
            send_buf[0..chunk.len()].copy_from_slice(chunk);
            self.stream.write(&send_buf).unwrap();
        } else if chunk.len() > 32 {
            println!("Tried to send a chunk that was too big.");
            return false;
        } else {
            self.stream.write(chunk).unwrap();
        }

        let ok = b"ok";

        // println!("Said {} waiting for reply", core::str::from_utf8(chunk).unwrap());
        let mut data = [0 as u8; 2]; // using 32 byte buffer

        match self.stream.read_exact(&mut data) {
            Ok(_) => {
                if &data == ok {
                    // println!("Reply is ok!");
                } else {
                    let text = from_utf8(&data).unwrap();
                    println!("Unexpected reply: {}", text);
                    return false;
                }
            }
            Err(e) => {
                println!("Failed to receive data: {}", e);
            }
        }
        return true;
    }
    pub fn speak(&mut self, msg: &str) -> bool {
        let mut iter = msg.as_bytes().chunks_exact(32);
        loop {
            if let Some(chunk) = iter.next() {
                self.send_chunk(chunk);
            } else {
                break;
            }
        }
        self.send_chunk(iter.remainder());
        return true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
