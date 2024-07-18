use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::process::Command;

use super::model::*;

/// Define some constants for pretty printing using ANSI colour codes.
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31;1m";
pub const RESET: &str = "\x1b[0m";
pub const GREY: &str = "\x1b[90;3m";

pub fn start(model: Arc<Mutex<Model>>) -> Result<(), ()> {
    println!("{BOLD}Enter search:{RESET}");
    let mut prev_results: Vec<(usize, String)> = Vec::new();
    
    loop {
        print!("{RED}> {RESET}");
        match io::stdout().flush() {
            Ok(()) => {
                // if writing stdout worked, get the next line of input
                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_n) => {
                        let input = input.trim_end();

                        if input == "exit" { break Ok(()); }
                        if input.split_whitespace()
                                .next()
                                .unwrap_or("") == "open" {
                            let num = input.split_whitespace().collect::<Vec<_>>()[1];
                            for r in &prev_results {
                                if r.0 == num.parse::<usize>().expect("not a number!") {
                                    println!("{GREY}Opening {}...{RESET}", &r.1);
                                    Command::new("xdg-open")
                                             .arg(&r.1)
                                             .spawn()
                                             .expect("xdg-open command failed to start");
                                }
                            }
                        }

                        if !input.starts_with("/") { continue; }
                        println!("{GREY}[{input}]{RESET}");
                        prev_results.clear();
                        let body = &input.chars().collect::<Vec<_>>();
                        let model = model.lock().unwrap();
                        let result = model.search_query(&body); 

                        let max = if result.len() > 30 { 30 as usize } else { result.len() };

                        let results = &result[..max];
                        for r in results {
                            if r.1 > 0.0 {
                                let count = results.iter().position(|x| x == r).unwrap() + 1;
                                let name = format!("{}", r.0.display());
                                println!(" {:>3} {} ({})", count, name, r.1);
                                
                                prev_results.push((count, name));
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
