extern crate actix_web;
extern crate crypto;
extern crate env_logger;
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate structopt;

use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::MetadataExt;

use actix_web::{http, server, App, HttpRequest, Json, Query, fs::NamedFile, middleware::Logger};
use structopt::StructOpt;

use scrib;


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
    scrib::add(&buf);
    Ok(Json(true))
}

#[derive(Debug, Deserialize)]
struct TagRequest {
    tag: String,
    target_ids: Vec<String>,
}

fn handle_tag(req: Json<TagRequest>) -> actix_web::Result<Json<bool>> {
    for target_id in req.target_ids.iter() {
        scrib::tag(&req.tag, &target_id);
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
    let mut obj_dir = scrib::get_scrib_home();
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

        let tags = scrib::tags_of(&id);

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
    scrib::init();
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
            let hash = scrib::add(&text);
            println!("{}", &hash);
        },
        Args::Tag { tag, hash } => scrib::tag(&tag, &hash),
        Args::Tags => scrib::tags(),
        Args::TagsOf { hash } => {
            for tag in scrib::tags_of(&hash) {
                println!("{}", &tag);
            }
        },
        Args::List { size } => scrib::list(size),
        Args::Serve => serve(),
    }
}
