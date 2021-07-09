use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::*;
use derive_more::{Display, Error};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Mutex;
use std::path::PathBuf;
use actix::{Addr, Actor};
use std::time::Instant;
use actix_web_actors::ws;
use crate::websocket_session_actor::{WebsocketSessionActor, MAIN_ROOM};

mod websocket_server_actor;
mod websocket_session_actor;

pub struct AppState {
    requested_song_ids: Vec<String>,
}

#[derive(Debug, Display, Error)]
#[display(fmt = "SongRequestError: {}", description)]
pub struct SongRequestError {
    description: &'static str,
}

// Use default implementation for `error_response()` method
impl error::ResponseError for SongRequestError {}

#[get("/")]
async fn index_service() -> Result<NamedFile> {
    let path: PathBuf = "songs.json".parse().unwrap();
    Ok(NamedFile::open(path)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSongRequestsResponse {
    requested_song_ids: Vec<String>,
}

#[get("/song-request")]
async fn list_song_requests_service(
    state: web::Data<Mutex<AppState>>,
) -> Result<web::Json<ListSongRequestsResponse>> {
    let state = state.lock().unwrap();

    Ok(web::Json(ListSongRequestsResponse {
        requested_song_ids: state.requested_song_ids.clone()
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSongRequestRequest {
    song_id: String,
}

#[post("/song-request")]
async fn create_song_request_service(
    create_song_request: web::Json<CreateSongRequestRequest>,
    app_state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> Result<web::Json<ListSongRequestsResponse>, SongRequestError> {
    let mut state = app_state.lock().unwrap();

    let position = state.requested_song_ids
        .iter()
        .position(|id| *id == create_song_request.song_id);

    return if let None = position {
        state.requested_song_ids.push(create_song_request.song_id.clone());

        websocket_server_actor_address.do_send(
            websocket_server_actor::BroadcastAppStateMessage {
                room_name: MAIN_ROOM.to_owned(),
            }
        );

        Ok(web::Json(ListSongRequestsResponse {
            requested_song_ids: state.requested_song_ids.clone()
        }))
    } else {
        Err(SongRequestError { description: "Existing song id." })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSongRequestRequest {
    song_id: String,
}

#[delete("/song-request")]
async fn delete_song_request_service(
    delete_song_request: web::Json<DeleteSongRequestRequest>,
    state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> Result<web::Json<ListSongRequestsResponse>, SongRequestError> {
    let mut state = state.lock().unwrap();

    let position = state.requested_song_ids
        .iter()
        .position(|id| *id == delete_song_request.song_id);

    return if let Some(position) = position {
        state.requested_song_ids.remove(position);

        websocket_server_actor_address.do_send(
            websocket_server_actor::BroadcastAppStateMessage {
                room_name: MAIN_ROOM.to_owned(),
            }
        );

        Ok(web::Json(ListSongRequestsResponse {
            requested_song_ids: state.requested_song_ids.clone()
        }))
    } else {
        Err(SongRequestError { description: "Invalid song id." })
    }
}

async fn websocket_service(
    request: HttpRequest,
    stream: web::Payload,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        WebsocketSessionActor {
            session_id: 0,
            last_heartbeat: Instant::now(),
            room_name: MAIN_ROOM.to_owned(),
            websocket_server_actor_address: websocket_server_actor_address.get_ref().clone(),
        },
        &request,
        stream,
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    let app_state = web::Data::new(Mutex::new(AppState {
        requested_song_ids: vec![],
    }));

    let websocket_server_actor_address = websocket_server_actor::WebsocketServerActor::new(app_state.clone()).start();

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .data(websocket_server_actor_address.clone())
            .service(index_service)
            .service(list_song_requests_service)
            .service(create_song_request_service)
            .service(delete_song_request_service)
            .service(web::resource("/ws/").to(websocket_service))
    })
        .bind(("0.0.0.0", port))?
        .run()
        .await
}
