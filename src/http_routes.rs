use actix::*;
use actix_web::*;
use actix_web_actors::ws;
use actix_files::NamedFile;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::websocket_session_actor::{WebsocketSessionActor, MAIN_ROOM};
use crate::{AppState, websocket_server_actor};

#[get("/")]
pub async fn index_service() -> Result<NamedFile> {
    let path: PathBuf = "songs.json".parse().unwrap();
    Ok(NamedFile::open(path)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSongRequestsResponse {
    requested_song_ids: Vec<String>,
}

#[get("/song-request")]
pub async fn list_song_requests_service(
    state: web::Data<Mutex<AppState>>,
) -> web::Json<ListSongRequestsResponse> {
    let state = state.lock().unwrap();

    web::Json(ListSongRequestsResponse {
        requested_song_ids: state.requested_song_ids.clone()
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSongRequestRequest {
    song_id: String,
}

#[post("/song-request")]
pub async fn create_song_request_service(
    create_song_request: web::Json<CreateSongRequestRequest>,
    app_state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<ListSongRequestsResponse> {
    let mut state = app_state.lock().unwrap();

    let position = state.requested_song_ids
        .iter()
        .position(|id| *id == create_song_request.song_id);

    if let None = position {
        state.requested_song_ids.push(create_song_request.song_id.clone());

        websocket_server_actor_address.do_send(
            websocket_server_actor::BroadcastAppStateMessage {
                room_name: MAIN_ROOM.to_owned(),
            }
        );
    }

    web::Json(ListSongRequestsResponse {
        requested_song_ids: state.requested_song_ids.clone()
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSongRequestRequest {
    song_id: String,
}

#[delete("/song-request")]
pub async fn delete_song_request_service(
    delete_song_request: web::Json<DeleteSongRequestRequest>,
    state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<ListSongRequestsResponse> {
    let mut state = state.lock().unwrap();

    let position = state.requested_song_ids
        .iter()
        .position(|id| *id == delete_song_request.song_id);

    if let Some(position) = position {
        state.requested_song_ids.remove(position);

        websocket_server_actor_address.do_send(
            websocket_server_actor::BroadcastAppStateMessage {
                room_name: MAIN_ROOM.to_owned(),
            }
        );
    }

    web::Json(ListSongRequestsResponse {
        requested_song_ids: state.requested_song_ids.clone()
    })
}

pub async fn websocket_service(
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