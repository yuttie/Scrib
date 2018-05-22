extern crate actix_web;
extern crate crypto;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::env;
use std::fs::{self, DirEntry, File};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::{symlink, MetadataExt};
use std::path::{PathBuf};

use actix_web::{http, server, App, HttpRequest, Json, Query, fs::NamedFile, middleware::Logger};
use structopt::StructOpt;



#[derive(Debug, StructOpt)]
#[structopt(name = "scrib", about = "Let's Scribble!")]
enum Args {
    #[structopt(name = "add")]
    Add {
        text: Vec<String>,
    },
    #[structopt(name = "tag")]
    Tag {
        tag: String,
        hash: String,
    },
    #[structopt(name = "tags")]
    Tags,
    #[structopt(name = "tags-of")]
    TagsOf {
        hash: String,
    },
    #[structopt(name = "list")]
    List {
        #[structopt(short = "n", long = "size")]
        size: Option<usize>,
    },
    #[structopt(name = "serve")]
    Serve,
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

fn list(size: Option<usize>) {
    let mut obj_dir = get_scrib_home();
    obj_dir.push("objects");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().unwrap().mtime();
        let mtime_b = b.metadata().unwrap().mtime();
        mtime_b.cmp(&mtime_a)
    });

    let entries: &[_] = match size {
        Some(n) => &entries[..n],
        None => &entries[..],
    };

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

fn serve() {
    const HOST_PORT: &str = "localhost:3000";
    server::new(
        || App::new()
            .middleware(Logger::default())
            .route("/", http::Method::GET, handle_root)
            .route("/add", http::Method::POST, handle_add)
            .route("/tag", http::Method::POST, handle_tag)
            .route("/list", http::Method::GET, handle_list))
        .bind(HOST_PORT).expect(&format!("Can not bind to {}", HOST_PORT))
        .run();
}

fn handle_root(_req: HttpRequest) -> actix_web::Result<NamedFile> {
    Ok(NamedFile::open("static/index.html")?)
}

fn handle_add(mut req: HttpRequest) -> actix_web::Result<Json<bool>> {
    let mut buf = String::new();
    req.read_to_string(&mut buf).unwrap();
    add(&buf);
    Ok(Json(true))
}

#[derive(Debug, Deserialize)]
struct TagRequest {
    tag: String,
    target_ids: Vec<String>,
}

fn handle_tag(req: Json<TagRequest>) -> actix_web::Result<Json<bool>> {
    for target_id in req.target_ids.iter() {
        tag(&req.tag, &target_id);
    }
    Ok(Json(true))
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    size: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Scribble {
    id:      String,
    content: String,
    tags:    Vec<String>,
}

fn handle_list(req: Query<ListRequest>) -> actix_web::Result<Json<Vec<Scribble>>> {
    let mut obj_dir = get_scrib_home();
    obj_dir.push("objects");
    let mut entries: Vec<_> = obj_dir.read_dir().unwrap().map(|entry| entry.unwrap()).collect();

    entries.sort_by(|a, b| {
        let mtime_a = a.metadata().unwrap().mtime();
        let mtime_b = b.metadata().unwrap().mtime();
        mtime_b.cmp(&mtime_a)
    });

    let entries: &[_] = match req.size {
        Some(n) => &entries[..n],
        None => &entries[..],
    };

    let mut scribbles: Vec<Scribble> = Vec::new();
    for entry in entries {
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

    Ok(Json(scribbles))
}

fn main() {
    env_logger::init();

    let args = Args::from_args();
    init();
    match args {
        Args::Add { text } => {
            let text = if text.is_empty() {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf).unwrap();
                buf
            }
            else {
                text.join(" ")
            };
            let hash = add(&text);
            println!("{}", &hash);
        },
        Args::Tag { tag, hash } => ::tag(&tag, &hash),
        Args::Tags => tags(),
        Args::TagsOf { hash } => {
            for tag in tags_of(&hash) {
                println!("{}", &tag);
            }
        },
        Args::List { size } => list(size),
        Args::Serve => serve(),
    }
}
