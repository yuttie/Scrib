use ::actix::prelude::*;
use diesel;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::{models, Result};

use self::models::{Scribble, Tag, Tagging};


pub struct DbExecutor(pub Pool<ConnectionManager<PgConnection>>);

impl Actor for DbExecutor {
    type Context = SyncContext<Self>;
}

pub struct CreateScribble {
    pub text: String,
}

impl Message for CreateScribble {
    type Result = Result<Scribble>;
}

pub struct UpdateScribble {
    pub scribble_id: i64,
    pub text: String,
}

impl Message for UpdateScribble {
    type Result = Result<Scribble>;
}

pub struct DeleteScribble {
    pub scribble_id: i64,
}

impl Message for DeleteScribble {
    type Result = Result<()>;
}

pub struct CreateTag {
    pub text: String,
}

impl Message for CreateTag {
    type Result =  Result<Tag>;
}


pub struct TagScribble {
    pub scribble_id: i64,
    pub tag_text: String,
}

impl Message for TagScribble {
    type Result = Result<Tagging>;
}

pub struct Tags;

impl Message for Tags {
    type Result = Result<Vec<Tag>>;
}

pub struct List {
    pub size: Option<usize>,
}

impl Message for List {
    type Result = Result<Vec<Scribble>>;
}

pub struct TagsOf {
    pub scribble_id: i64,
}

impl Message for TagsOf {
    type Result = Result<Vec<Tag>>;
}

impl Handler<CreateScribble> for DbExecutor {
    type Result = Result<Scribble>;

    fn handle(&mut self, msg: CreateScribble, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::create_scribble(conn, msg.text.as_str())
    }
}

impl Handler<UpdateScribble> for DbExecutor {
    type Result = Result<Scribble>;

    fn handle(&mut self, msg: UpdateScribble, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::update_scribble(conn, msg.scribble_id, msg.text.as_str())
    }
}

impl Handler<DeleteScribble> for DbExecutor {
    type Result = Result<()>;

    fn handle(&mut self, msg: DeleteScribble, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::delete_scribble(conn, msg.scribble_id)
    }
}

impl Handler<CreateTag> for DbExecutor {
    type Result = Result<Tag>;

    fn handle(&mut self, msg: CreateTag, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::create_tag(conn, msg.text.as_str())
    }
}

impl Handler<TagScribble> for DbExecutor {
    type Result = Result<Tagging>;

    fn handle(&mut self, msg: TagScribble, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::tag_scribble(conn, msg.scribble_id, msg.tag_text.as_str())
    }
}

impl Handler<Tags> for DbExecutor {
    type Result = Result<Vec<Tag>>;

    fn handle(&mut self, msg: Tags, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::tags(conn)
    }
}

impl Handler<List> for DbExecutor {
    type Result = Result<Vec<Scribble>>;

    fn handle(&mut self, msg: List, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::list(conn, msg.size)
    }
}

impl Handler<TagsOf> for DbExecutor {
    type Result = Result<Vec<Tag>>;

    fn handle(&mut self, msg: TagsOf, _: &mut Self::Context) -> Self::Result {
        let conn: &PgConnection = &self.0.get().unwrap();
        crate::tags_of(conn, msg.scribble_id)
    }
}
