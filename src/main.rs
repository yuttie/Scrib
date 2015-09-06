#![feature(path_ext)]

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
use std::os::unix::fs::{symlink, MetadataExt};
use std::path::PathBuf;

const USAGE: &'static str = "
Fine-grained.

Usage:
    fine add [<text>...]
    fine tag <tag> <hash>
    fine list
    fine (-h | --help)
    fine --version

Options:
    -h, --help  Show this message.
    --version   Show version.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_add:  bool,
    cmd_tag:  bool,
    cmd_list: bool,
    arg_text: Vec<String>,
    arg_tag:  String,
    arg_hash: String,
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
    pathbuf.pop();

    pathbuf.push("tags");
    fs::create_dir_all(pathbuf.as_path()).unwrap();
    pathbuf.pop();
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

fn lookup_hash(hash: &str) -> Result<PathBuf, &str> {
    let mut pathbuf = get_fine_home();
    pathbuf.push("objects");

    let mut candidates: Vec<PathBuf> = pathbuf.read_dir().unwrap().filter_map(|entry| {
        let entry = entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.starts_with(hash) {
            Some(entry.path())
        }
        else {
            None
        }
    }).collect();
    if candidates.len() == 0 {
        println!("Found no candidate.");
        Err("Found no candidate.")
    }
    else if candidates.len() == 1 {
        println!("Found a single candidate.");
        Ok(candidates.swap_remove(0))
    }
    else {
        Err("Too many candidates.")
    }
}

fn tag(tag: &str, hash: &str) {
    let hash = lookup_hash(&hash).unwrap().file_name().unwrap().to_owned();

    let mut src = PathBuf::from("../../objects/");
    src.push(&hash);

    let mut dest = get_fine_home();
    dest.push("tags");
    dest.push(&tag);
    fs::create_dir_all(dest.as_path()).unwrap();
    dest.push(&hash);

    symlink(&src, &dest).unwrap();
}

fn list() {
    let mut obj_dir = get_fine_home();
    obj_dir.push("objects");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let ctime_a = a.metadata().unwrap().ctime();
        let ctime_b = b.metadata().unwrap().ctime();
        ctime_b.cmp(&ctime_a)
    });

    for entry in entries {
        let file_name = entry.file_name();

        let content = {
            let mut file = File::open(entry.path()).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();
            content
        };

        println!("{} {}",
            &file_name.to_str().unwrap()[0..8],
            &content);
    }
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
    else if args.cmd_tag {
        tag(&args.arg_tag, &args.arg_hash);
    }
    else if args.cmd_list {
        list();
    }
}
