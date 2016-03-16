extern crate crypto;
extern crate docopt;
extern crate rustc_serialize;
extern crate iron;
extern crate router;
extern crate handlebars_iron as hbs;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::{symlink, MetadataExt};
use std::path::PathBuf;
use std::sync::Arc;

use docopt::Docopt;
use iron::prelude::*;
use iron::status;
use router::Router;
use hbs::{Template, HandlebarsEngine, DirectorySource};
#[cfg(feature = "watch")]
use hbs::Watchable;



const USAGE: &'static str = "
Let's Scribble!

Usage:
    scrib add [<text>...]
    scrib tag <tag> <hash>
    scrib list
    scrib serve
    scrib (-h | --help)
    scrib --version

Options:
    -h, --help  Show this message.
    --version   Show version.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_add:   bool,
    cmd_tag:   bool,
    cmd_list:  bool,
    cmd_serve: bool,
    arg_text:  Vec<String>,
    arg_tag:   String,
    arg_hash:  String,
}

fn get_scrib_home() -> PathBuf {
    let mut pathbuf = env::home_dir().unwrap();
    pathbuf.push(".scrib");
    pathbuf
}

fn init() {
    let mut pathbuf = get_scrib_home();
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
        let mut pathbuf = get_scrib_home();
        pathbuf.push("objects");
        pathbuf.push(&digest);

        File::create(&pathbuf).unwrap()
    };
    file.write_all(text.as_bytes()).unwrap();

    println!("{}", &digest);
}

fn lookup_hash(hash: &str) -> Result<PathBuf, &str> {
    let mut pathbuf = get_scrib_home();
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

    let mut dest = get_scrib_home();
    dest.push("tags");
    dest.push(&tag);
    fs::create_dir_all(dest.as_path()).unwrap();
    dest.push(&hash);

    symlink(&src, &dest).unwrap();
}

fn list() {
    let mut obj_dir = get_scrib_home();
    obj_dir.push("objects");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().unwrap().mtime();
        let mtime_b = b.metadata().unwrap().mtime();
        mtime_b.cmp(&mtime_a)
    });

    for entry in entries {
        let file_name = entry.file_name();

        let content = {
            let file = File::open(entry.path()).unwrap();
            let mut buf: Vec<u8> = Vec::new();
            file.take(80).read_to_end(&mut buf).unwrap();
            match String::from_utf8(buf.clone()) {
                Ok(string) => string.lines().next().unwrap().to_owned(),
                Err(_) => {
                    let mut string = String::new();
                    for b in &buf[0..20] {
                        string.push_str(&format!("\\x{:x}", b));
                    }
                    string
                },
            }
        };

        println!("{} {}",
            &file_name.to_str().unwrap()[0..8],
            &content);
    }
}

fn serve() {
    let mut router = Router::new();

    router.get("/", |_: &mut Request| {
        Ok(Response::with((status::Ok, Template::new("index", ()))))
    });

    router.get("/list", |_: &mut Request| {
        let mut obj_dir = get_scrib_home();
        obj_dir.push("objects");
        let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

        entries.sort_by(|a, b| {
            let mtime_a = a.metadata().unwrap().mtime();
            let mtime_b = b.metadata().unwrap().mtime();
            mtime_b.cmp(&mtime_a)
        });

        let mut json = String::from("[");
        for entry in &entries {
            let file_name = entry.file_name();

            let content = {
                let file = File::open(entry.path()).unwrap();
                let mut buf: Vec<u8> = Vec::new();
                file.take(80).read_to_end(&mut buf).unwrap();
                match String::from_utf8(buf.clone()) {
                    Ok(string) => string.lines().next().unwrap().to_owned(),
                    Err(_) => {
                        let mut string = String::new();
                        for b in &buf[0..20] {
                            string.push_str(&format!("\\\\x{:x}", b));
                        }
                        string
                    },
                }
            };

            json.push_str(&format!(r#"{{"id":"{}","content":"{}"}},"#,
                &file_name.to_str().unwrap(),
                &content));
        }
        if entries.len() > 1 {
            json.pop();
        }
        json.push(']');
        Ok(Response::with((status::Ok, json)))
    });

    let mut chain = Chain::new(router);
    let mut hbse = HandlebarsEngine::new2();
    hbse.add(Box::new(DirectorySource::new("templates/", ".hbs")));
    if let Err(r) = hbse.reload() {
        panic!("{}", r.description());
    }

    let hbse_ref = Arc::new(hbse);
    hbse_ref.watch("templates/");

    chain.link_after(hbse_ref);
    Iron::new(chain).http("localhost:3000").unwrap();
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
    else if args.cmd_serve {
        serve();
    }
}
