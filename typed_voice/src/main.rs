use std::io::{self, BufRead};

use voice::Voice;

fn main() {
    let mut v = Voice::new().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        if v.speak(line.unwrap().as_str()) {
            println!("Success")
        }
    }
}
