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
    scrib tags-of <hash>
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
    cmd_tags_of: bool,
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

#[derive(Debug)]
struct KeepNote {
    heading:     String,
    title:       String,
    content:     String,
    attachments: Vec<String>,
    labels:      Vec<String>,
}

impl serde::Serialize for KeepNote {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer
    {
        serializer.serialize_struct("KeepNote", KeepNoteMapVisitor {
            value: self,
            state: 0,
        })
    }
}

struct KeepNoteMapVisitor<'a> {
    value: &'a KeepNote,
    state: u8,
}

impl<'a> serde::ser::MapVisitor for KeepNoteMapVisitor<'a> {
    fn visit<S>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error>
        where S: serde::Serializer
    {
        match self.state {
            0 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("heading", &self.value.heading))))
            },
            1 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("title", &self.value.title))))
            },
            2 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("content", &self.value.content))))
            },
            3 => {
                self.state += 1;
                Ok(Some(try!(serializer.serialize_struct_elt("attachments", &self.value.attachments))))
            },
            _ => {
                Ok(None)
            },
        }
    }
}

fn import_keep<P: AsRef<Path>>(fp: P) {
    let mut html: String = String::new();
    let mut file = File::open(fp).unwrap();
    file.read_to_string(&mut html).unwrap();
    let doc = Html::parse_document(&html);
    let note = parse_document(doc);

    let json = serde_json::to_string_pretty(&note).unwrap();
    let hash = add(&json);
    println!("{}", &hash);

    // Parse the heading as a local time
    let note_mtime =
        Local.datetime_from_str(&note.heading, "%b %d, %Y, %I:%M:%S %p").unwrap();
    let timestamp: i64 = note_mtime.timestamp();
    // Use it as timestamps of the object file
    let mtime = FileTime::from_seconds_since_1970(timestamp as u64, 0);
    let mut obj_path = get_scrib_home();
    obj_path.push("objects");
    obj_path.push(&hash);
    set_file_times(&obj_path, mtime, mtime).unwrap();

    tag("parsable-as-json", &hash);
    tag("imported-from-google-keep", &hash);
    // Add note's labels as tags
    for label in note.labels {
        tag(&label, &hash);
    }

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

    fn parse_labels(elem: ElementRef) -> Vec<String> {
        let mut labels: Vec<String> = Vec::new();
        // div.labels > span.label*
        let label_selector = Selector::parse(".label").unwrap();
        let label_elems = elem.select(&label_selector);
        for label_elem in label_elems {
            let label = collect_texts(label_elem);
            labels.push(label);
        }
        labels
    }

    fn parse_document(doc: Html) -> KeepNote {
        let heading_selector     = Selector::parse(".note .heading").unwrap();
        let title_selector       = Selector::parse(".note .title").unwrap();
        let content_selector     = Selector::parse(".note .content").unwrap();
        let attachments_selector = Selector::parse(".note .attachments").unwrap();
        let labels_selector      = Selector::parse(".note .labels").unwrap();

        let mut heading_elems     = doc.select(&heading_selector);
        let mut title_elems       = doc.select(&title_selector);
        let mut content_elems     = doc.select(&content_selector);
        let mut attachments_elems = doc.select(&attachments_selector);
        let mut labels_elems      = doc.select(&labels_selector);

        let heading = parse_heading(heading_elems.next().unwrap());
        let title = match title_elems.next() {
            Some(e) => parse_title(e),
            None => "".to_string(),
        };
        let content = parse_content(content_elems.next().unwrap());
        let attachments = match attachments_elems.next() {
            Some(e) => parse_attachments(e),
            None => vec![],
        };
        let labels = match labels_elems.next() {
            Some(e) => parse_labels(e),
            None => vec![],
        };

        let note = KeepNote {
            heading:     heading,
            title:       title,
            content:     content,
            attachments: attachments,
            labels:      labels,
        };
        note
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
        let hash = add(&text);
        println!("{}", &hash);
    }
    else if args.cmd_tag {
        tag(&args.arg_tag, &args.arg_hash);
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
    else if args.cmd_import_keep {
        for fp in args.arg_file {
            println!("Importing from {}...", fp);
            import_keep(fp);
        }
    }
}
