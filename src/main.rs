extern crate chrono;
extern crate crypto;
extern crate docopt;
extern crate filetime;
extern crate rustc_serialize;
extern crate iron;
extern crate router;
extern crate handlebars_iron as hbs;
extern crate scraper;
extern crate serde;
extern crate serde_json;

use chrono::*;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use filetime::*;
use std::env;
use std::error::Error;
use std::fs::{self, DirEntry, File};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::{symlink, MetadataExt};
use std::path::{Path, PathBuf};
#[cfg(feature = "watch")]
use std::sync::Arc;

use docopt::Docopt;
use iron::prelude::*;
use iron::status;
use router::Router;
use hbs::{Template, HandlebarsEngine, DirectorySource};
#[cfg(feature = "watch")]
use hbs::Watchable;
use scraper::{ElementRef, Html, Selector};
use scraper::node::Node;



const USAGE: &'static str = "
Let's Scribble!

Usage:
    scrib add [<text>...]
    scrib tag <tag> <hash>
    scrib tags
    scrib tags-of <hash>
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
    cmd_tags:  bool,
    cmd_tags_of: bool,
    cmd_list:  bool,
    cmd_serve: bool,
    arg_text:  Vec<String>,
    arg_tag:   String,
    arg_hash:  String,
    arg_file:  Vec<String>,
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

fn add(text: &str) -> String {
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

    digest
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

    symlink(&src, &dest).unwrap_or(());
    println!("Added tag '{}' to {:?}", &tag, &hash);
}

fn tags() {
    let mut obj_dir = get_scrib_home();
    obj_dir.push("tags");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().unwrap().mtime();
        let mtime_b = b.metadata().unwrap().mtime();
        mtime_b.cmp(&mtime_a)
    });

    for entry in entries {
        let dir_name = entry.file_name();
        println!("{}", &dir_name.to_str().unwrap());
    }
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

fn tags_of(hash: &str) -> Vec<String> {
    let hash = lookup_hash(&hash).unwrap().file_name().unwrap().to_owned();

    let mut tags_dir = get_scrib_home();
    tags_dir.push("tags");
    let tags_dir_entries: Vec<DirEntry> = tags_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    let mut tags = Vec::new();
    for tag_dir_entry in tags_dir_entries {
        let tag = tag_dir_entry.file_name().into_string().unwrap();
        let mut tag_path = tag_dir_entry.path();
        tag_path.push(&hash);

        if tag_path.exists() {
            tags.push(tag.to_owned());
        }
    }

    tags
}

#[cfg(feature = "watch")]
fn serve() {
    let mut router = Router::new();

    router.get("/", handle_root);
    router.post("/add", handle_add);
    router.get("/list", handle_list);

    let mut chain = Chain::new(router);
    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new("templates/", ".hbs")));
    if let Err(r) = hbse.reload() {
        panic!("{}", r.description());
    }

    let hbse_ref = Arc::new(hbse);
    hbse_ref.watch("templates/");

    chain.link_after(hbse_ref);
    Iron::new(chain).http("localhost:3000").unwrap();
}

#[cfg(not(feature = "watch"))]
fn serve() {
    let mut router = Router::new();

    router.get("/", handle_root);
    router.post("/add", handle_add);
    router.get("/list", handle_list);

    let mut chain = Chain::new(router);
    let mut hbse = HandlebarsEngine::new();
    hbse.add(Box::new(DirectorySource::new("templates/", ".hbs")));
    if let Err(r) = hbse.reload() {
        panic!("{}", r.description());
    }

    chain.link_after(hbse);
    Iron::new(chain).http("localhost:3000").unwrap();
}

fn handle_root(_: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, Template::new("index", ()))))
}

fn handle_add(req: &mut Request) -> IronResult<Response> {
    let mut buf = String::new();
    req.body.read_to_string(&mut buf).unwrap();
    add(&buf);
    Ok(Response::with((status::Ok, "true")))
}

#[derive(Debug)]
struct Scribble {
    id:      String,
    content: String,
    tags:    Vec<String>,
}

impl serde::Serialize for Scribble {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer
    {
        serializer.serialize_struct("Scribble", ScribbleMapVisitor {
            value: self,
            state: 0,
        })
    }
}

struct ScribbleMapVisitor<'a> {
    value: &'a Scribble,
    state: u8,
}

impl<'a> serde::ser::MapVisitor for ScribbleMapVisitor<'a> {
    fn visit<S>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error>
        where S: serde::Serializer
    {
        match self.state {
            0 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("id", &self.value.id))))
            },
            1 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("content", &self.value.content))))
            },
            2 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("tags", &self.value.tags))))
            },
            _ => {
                Ok(None)
            },
        }
    }
}

fn handle_list(_: &mut Request) -> IronResult<Response> {
    let mut obj_dir = get_scrib_home();
    obj_dir.push("objects");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().unwrap().mtime();
        let mtime_b = b.metadata().unwrap().mtime();
        mtime_b.cmp(&mtime_a)
    });

    let mut scribbles: Vec<Scribble> = Vec::new();
    for entry in &entries {
        let id = entry.file_name().into_string().unwrap();

        let mut file = File::open(entry.path()).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        let content = match String::from_utf8(buf.clone()) {
            Ok(content) => content,
            Err(_) => {
                let mut content = String::new();
                for b in &buf[0..20] {
                    content.push_str(&format!("\\u{:04x}", b));
                }
                content
            },
        };

        let tags = tags_of(&id);

        let scribble = Scribble {
            id:      id,
            content: content,
            tags:    tags,
        };
        scribbles.push(scribble);
    }

    let json = serde_json::to_string(&scribbles).unwrap();
    Ok(Response::with((status::Ok, json)))
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
        let hash = add(&text);
        println!("{}", &hash);
    }
    else if args.cmd_tag {
        tag(&args.arg_tag, &args.arg_hash);
    }
    else if args.cmd_tags {
        tags();
    }
    else if args.cmd_tags_of {
        for tag in tags_of(&args.arg_hash) {
            println!("{}", &tag);
        }
    }
    else if args.cmd_list {
        list();
    }
    else if args.cmd_serve {
        serve();
    }
}
