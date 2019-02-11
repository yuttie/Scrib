pub mod db;

use actix::prelude::*;
use actix_web::{http, server, App, HttpRequest, HttpResponse, AsyncResponder, FutureResponse, State, Json, Query, Result, fs::NamedFile, middleware::Logger, middleware::cors::Cors};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use futures::Future;
use serde_json::json;

use db::{CreateScribble, UpdateScribble, TagScribble, List};


struct AppState {
    db: Addr<db::DbExecutor>,
}

pub fn start(pool: Pool<ConnectionManager<SqliteConnection>>) {
    const HOST_PORT: &str = "localhost:3000";
    let sys = actix::System::new("diesel-example");
    let addr = SyncArbiter::start(3, move || db::DbExecutor(pool.clone()));
    server::new(move || {
        App::with_state(AppState { db: addr.clone() })
            .middleware(Logger::default())
            .configure(|app| {
                Cors::for_app(app)
                    .allowed_origin("http://localhost:8080")
                    .resource("/", |r| r.method(http::Method::GET).f(handle_root))
                    .resource("/add", |r| r.method(http::Method::POST).with(handle_add))
                    .resource("/update", |r| r.method(http::Method::POST).with(handle_update))
                    .resource("/tag", |r| r.method(http::Method::POST).with(handle_tag))
                    .resource("/list", |r| r.method(http::Method::GET).with(handle_list))
                    .register()
            })
    }).bind(HOST_PORT)
      .expect(&format!("Can not bind to {}", HOST_PORT))
      .start();
    let _ = sys.run();
}

fn handle_root(_req: &HttpRequest<AppState>) -> Result<NamedFile> {
    Ok(NamedFile::open("static/index.html")?)
}

#[derive(Debug, Deserialize)]
struct AddRequest {
    text: String,
}

fn handle_add((req, state): (Json<AddRequest>, State<AppState>)) -> FutureResponse<HttpResponse> {
    state
        .db
        .send(CreateScribble {
            text: req.text.to_owned(),
        })
        .from_err()
        .and_then(|res| match res {
            Ok(scribble) => Ok(HttpResponse::Ok().json(scribble)),
            Err(_) => Ok(HttpResponse::InternalServerError().into()),
        })
        .responder()
}

#[derive(Debug, Deserialize)]
struct UpdateRequest {
    scribble_id: i64,
    text: String,
}

fn handle_update((req, state): (Json<UpdateRequest>, State<AppState>)) -> FutureResponse<HttpResponse> {
    state
        .db
        .send(UpdateScribble {
            scribble_id: req.scribble_id,
            text: req.text.to_owned(),
        })
        .from_err()
        .and_then(|res| match res {
            Ok(scribble) => Ok(HttpResponse::Ok().json(scribble)),
            Err(_) => Ok(HttpResponse::InternalServerError().into()),
        })
        .responder()
}

#[derive(Debug, Deserialize)]
struct TagRequest {
    scribble_id: i64,
    tag_text: String,
}

fn handle_tag((req, state): (Json<TagRequest>, State<AppState>)) -> FutureResponse<HttpResponse> {
    state
        .db
        .send(TagScribble {
            scribble_id: req.scribble_id,
            tag_text: req.tag_text.to_owned(),
        })
        .from_err()
        .and_then(|res| match res {
            Ok(tagging) => Ok(HttpResponse::Ok().json(tagging)),
            Err(crate::Error::AlreadyTagged) => Ok(HttpResponse::Ok().json(json!({
                "error": {
                    "type": "AlreadyTagged",
                },
            }))),
            Err(_) => Ok(HttpResponse::InternalServerError().into()),
        })
        .responder()
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    size: Option<usize>,
}

fn handle_list((req, state): (Query<ListRequest>, State<AppState>)) -> FutureResponse<HttpResponse> {
    state
        .db
        .send(List {
            size: req.size,
        })
        .from_err()
        .and_then(|res| match res {
            Ok(scribbles) => Ok(HttpResponse::Ok().json(scribbles)),
            Err(_) => Ok(HttpResponse::InternalServerError().into()),
        })
        .responder()
}
