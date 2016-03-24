extern crate crypto;
extern crate docopt;
extern crate rustc_serialize;
extern crate iron;
extern crate router;
extern crate handlebars_iron as hbs;
extern crate scraper;
extern crate serde;
extern crate serde_json;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::{symlink, MetadataExt};
use std::path::PathBuf;
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
    scrib list
    scrib serve
    scrib import-keep <file>...
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
    cmd_import_keep: bool,
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

#[cfg(feature = "watch")]
fn serve() {
    let mut router = Router::new();

    router.get("/", handle_root);
    router.post("/add", handle_add);
    router.get("/list", handle_list);

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

#[cfg(not(feature = "watch"))]
fn serve() {
    let mut router = Router::new();

    router.get("/", handle_root);
    router.post("/add", handle_add);
    router.get("/list", handle_list);

    let mut chain = Chain::new(router);
    let mut hbse = HandlebarsEngine::new2();
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

fn handle_list(_: &mut Request) -> IronResult<Response> {
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

        let mut file = File::open(entry.path()).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        match String::from_utf8(buf.clone()) {
            Ok(content) => {
                json.push_str(&format!(r#"{{"id":{},"content":{}}},"#,
                                        &serde_json::to_string(&file_name.to_str().unwrap()).unwrap(),
                                        &serde_json::to_string(&content).unwrap()));
            },
            Err(_) => {
                let mut content = String::new();
                for b in &buf[0..20] {
                    content.push_str(&format!("\\u{:04x}", b));
                }
                json.push_str(&format!(r#"{{"id":{},"content":"{}"}},"#,
                                        &serde_json::to_string(&file_name.to_str().unwrap()).unwrap(),
                                        &content));
            },
        }
    }
    if entries.len() > 1 {
        json.pop();
    }
    json.push(']');
    Ok(Response::with((status::Ok, json)))
}

fn import_keep(fp: String) {
    let mut html: String = String::new();
    let mut file = File::open(fp).unwrap();
    file.read_to_string(&mut html).unwrap();
    let document = Html::parse_fragment(&html);

    let heading_selector     = Selector::parse(".note .heading").unwrap();
    let title_selector       = Selector::parse(".note .title").unwrap();
    let content_selector     = Selector::parse(".note .content").unwrap();
    let attachments_selector = Selector::parse(".note .attachments").unwrap();

    let mut heading     = document.select(&heading_selector);
    let mut title       = document.select(&title_selector);
    let mut content     = document.select(&content_selector);
    let mut attachments = document.select(&attachments_selector);

    fn collect_texts(elem: ElementRef) -> String {
        let mut text = String::new();
        for child in elem.children() {
            match *child.value() {
                Node::Text(ref t) => text.push_str(&t),
                Node::Element(ref e) => {
                    if e.name.local.as_ref() == "br" {
                        text.push('\n');
                    }
                    else {
                        text.push_str(&collect_texts(ElementRef::wrap(child).unwrap()))
                    }
                },
                _ => (),
            }
        }
        text
    }

    fn parse_heading(elem: ElementRef) -> String {
        collect_texts(elem).trim().to_owned()
    }

    fn parse_title(elem: ElementRef) -> String {
        collect_texts(elem)
    }

    fn parse_content(elem: ElementRef) -> String {
        // div.content > (div.listitem > div.bullet + div.text)*
        let listitem_selector = Selector::parse(".listitem").unwrap();
        let mut listitems = elem.select(&listitem_selector).peekable();
        if listitems.peek().is_some() {
            // .content is a list
            let mut content = String::new();
            let bullet_selector = Selector::parse(".bullet").unwrap();
            let text_selector = Selector::parse(".text").unwrap();
            for listitem in listitems {
                let bullet = listitem.select(&bullet_selector).next().unwrap();
                let text = listitem.select(&text_selector).next().unwrap();
                content.push_str(&collect_texts(bullet));
                content.push(' ');
                content.push_str(&collect_texts(text));
                content.push('\n');
            }
            content
        }
        else {
            collect_texts(elem)
        }
    }

    fn parse_attachments(elem: ElementRef) -> Vec<String> {
        let mut attachments: Vec<String> = Vec::new();
        // div.attachments > ul > (li > img)*
        let li_selector = Selector::parse("ul > li").unwrap();
        let lis = elem.select(&li_selector);
        for li in lis {
            let img_selector = Selector::parse("img").unwrap();
            let mut imgs = li.select(&img_selector);
            match imgs.next() {
                Some(img) => attachments.push(img.value().attr("src").unwrap().to_owned()),
                None => (),
            }
        }
        attachments
    }

    println!("{}", &parse_heading(heading.next().unwrap()));
    println!("{}", &parse_title(title.next().unwrap()));
    println!("{}", &parse_content(content.next().unwrap()));
    match attachments.next() {
        Some(e) => println!("{:?}", &parse_attachments(e)),
        None => (),
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
    else if args.cmd_serve {
        serve();
    }
    else if args.cmd_import_keep {
        for fp in args.arg_file {
            println!("Importing from {}...", fp);
            import_keep(fp);
        }
    }
}
