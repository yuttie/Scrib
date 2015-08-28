extern crate crypto;
extern crate docopt;
extern crate rustc_serialize;

use docopt::Docopt;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::env;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;

const USAGE: &'static str = "
Fine-grained.

Usage:
    fine add [<text>...]
    fine (-h | --help)
    fine --version

Options:
    -h, --help  Show this message.
    --version   Show version.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_add: bool,
    arg_text: Vec<String>,
}

fn get_fine_home() -> PathBuf {
    let mut pathbuf = env::home_dir().unwrap();
    pathbuf.push(".fine");
    pathbuf
}

fn init() {
    let mut pathbuf = get_fine_home();
    fs::create_dir_all(pathbuf.as_path()).unwrap();

    pathbuf.push("objects");
    fs::create_dir_all(pathbuf.as_path()).unwrap();
}

fn add(text: &str) {
    let digest = {
        let mut hasher = Sha256::new();
        hasher.input_str(&text);
        hasher.result_str()
    };

    let mut file = {
        let mut pathbuf = get_fine_home();
        pathbuf.push("objects");
        pathbuf.push(&digest);

        File::create(&pathbuf).unwrap()
    };
    file.write_all(text.as_bytes()).unwrap();

    println!("{}", &digest);
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.decode())
                            .unwrap_or_else(|e| e.exit());
    init();
    if args.cmd_add {
        let text = if args.arg_text.is_empty() {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).unwrap();
            buf
        }
        else {
            args.arg_text.join(" ")
        };
        add(&text);
    }
}
