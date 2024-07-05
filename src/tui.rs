use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use super::model::*;

/// Define some constants for pretty printing using ANSI colour codes.
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31;1m";
pub const RESET: &str = "\x1b[0m";
pub const GREY: &str = "\x1b[90;3m";

pub fn start(model: Arc<Mutex<Model>>) -> Result<(), ()> {
    println!("{BOLD}Enter search:{RESET}");
    loop {
        print!("{RED}> {RESET}");
        match io::stdout().flush() {
            Ok(()) => {
                // if writing stdout worked, get the next line of input
                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_n) => {
                        let input = input.trim_end();
                        println!("{GREY}[{input}]{RESET}");
                        let body = &input.chars().collect::<Vec<_>>();
                        let model = model.lock().unwrap();
                        let result = model.search_query(&body); 

                        let max = if result.len() > 30 { 30 as usize } else { result.len() };

                        for r in &result[..max] {
                            if r.1 > 0.0 {
                                println!(" - {} ({})", r.0.display(), r.1);
                            }
                        }
                        println!("");
                    }
                    Err(e) => panic!("{e}"),
                }
            }
            Err(e) => panic!("{e}"),
        }
    }
}
