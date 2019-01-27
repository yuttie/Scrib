extern crate actix_web;
extern crate env_logger;
extern crate log;
extern crate structopt;

use std::io;
use std::io::prelude::*;
use std::os::unix::fs::MetadataExt;

use diesel::prelude::*;
use structopt::StructOpt;

use scrib::{self,models};


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
        scribble_id: i64,
    },
    #[structopt(name = "tags")]
    Tags,
    #[structopt(name = "tags-of")]
    TagsOf {
        scribble_id: i64,
    },
    #[structopt(name = "list")]
    List {
        #[structopt(short = "n", long = "size")]
        size: Option<usize>,
    },
    #[structopt(name = "serve")]
    Serve,
}

fn main() {
    env_logger::init();

    let args = Args::from_args();
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

            let conn = scrib::establish_connection();
            let hash = scrib::create_scribble(&conn, &text);
        },
        Args::Tag { tag, scribble_id } => {
            let conn = scrib::establish_connection();
            scrib::tag_scribble(&conn, scribble_id, &tag);
        },
        Args::Tags => {
            let conn = scrib::establish_connection();
            scrib::tags(&conn);
        },
        Args::TagsOf { scribble_id } => {
            let conn = scrib::establish_connection();
            for tag in scrib::tags_of(&conn, scribble_id).unwrap() {
                println!("{}", &tag.text);
            }
        },
        Args::List { size } => {
            let conn = scrib::establish_connection();
            scrib::list(&conn, size);
        },
        Args::Serve => {
            let pool = scrib::new_connection_pool();
            scrib::server::start(pool);
        },
    }
}
