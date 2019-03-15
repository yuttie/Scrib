extern crate actix_web;
extern crate env_logger;
extern crate log;
extern crate structopt;

use std::io;
use std::io::prelude::*;
use std::os::unix::fs::MetadataExt;

use diesel::prelude::*;
use structopt::StructOpt;

use forghetti::{self,models};


#[derive(Debug, StructOpt)]
#[structopt(name = "forghetti", about = "Scribble and forget it!")]
enum Args {
    #[structopt(name = "add")]
    Add {
        text: Vec<String>,
    },
    #[structopt(name = "update")]
    Update {
        scribble_id: i64,
        text: Vec<String>,
    },
    #[structopt(name = "delete")]
    Delete {
        scribble_id: i64,
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
    Serve {
        #[structopt(long = "host", default_value = "0.0.0.0")]
        host: String,
        #[structopt(name = "PORT")]
        port: u16,
    },
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

            let conn = forghetti::establish_connection();
            forghetti::create_scribble(&conn, &text).unwrap();
        },
        Args::Update { scribble_id, text } => {
            let text = if text.is_empty() {
                let mut buf = String::new();
                io::stdin().read_to_string(&mut buf).unwrap();
                buf
            }
            else {
                text.join(" ")
            };

            let conn = forghetti::establish_connection();
            forghetti::update_scribble(&conn, scribble_id, &text).unwrap();
        },
        Args::Delete { scribble_id } => {
            let conn = forghetti::establish_connection();
            forghetti::delete_scribble(&conn, scribble_id).unwrap();
        },
        Args::Tag { tag, scribble_id } => {
            let conn = forghetti::establish_connection();
            forghetti::tag_scribble(&conn, scribble_id, &tag).unwrap();
        },
        Args::Tags => {
            let conn = forghetti::establish_connection();
            for tag in forghetti::tags(&conn).unwrap() {
                println!("{}", &tag.text);
            }
        },
        Args::TagsOf { scribble_id } => {
            let conn = forghetti::establish_connection();
            for tag in forghetti::tags_of(&conn, scribble_id).unwrap() {
                println!("{}", &tag.text);
            }
        },
        Args::List { size } => {
            let conn = forghetti::establish_connection();
            for scribble in forghetti::list(&conn, size).unwrap() {
                println!("{:19}: {:?}", scribble.id, &scribble.text);
            }
        },
        Args::Serve { host, port } => {
            let pool = forghetti::new_connection_pool();
            forghetti::server::start(&host, port, pool);
        },
    }
}
