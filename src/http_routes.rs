use actix::*;
use actix_files::NamedFile;
use actix_web::*;
use actix_web_actors::ws;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use serde::Deserialize;

use crate::websocket_session_actor::WebsocketSessionActor;
use crate::{websocket_server_actor, AppState, ArrangementType, Playlist, SongRequest};

#[get("/{user_id}/songs")]
pub async fn list_songs(user_id: web::Path<String>) -> Result<NamedFile> {
    let filename = format!("{}.json", user_id.into_inner());
    let path: PathBuf = filename.parse().unwrap();
    Ok(NamedFile::open(path)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistUpdate {
    song_requests_enabled: bool,
    song_arrangements: Vec<ArrangementType>,
}

#[put("/{user_id}/songs")]
pub async fn update_playlist(
    user_id: web::Path<String>,
    playlist_update: web::Json<PlaylistUpdate>,
    app_state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<Playlist> {
    let user_id = user_id.into_inner();
    let mut state = app_state.lock().unwrap();

    let playlist = state
        .song_requests_by_user_id
        .entry(user_id.to_owned())
        .or_insert_with(|| Playlist {
            song_requests_enabled: false,
            song_arrangements: vec![
                ArrangementType::Lead,
                ArrangementType::Rhythm,
                ArrangementType::Bass,
                ArrangementType::Drums,
                ArrangementType::Vocals,
            ],
            song_requests: vec![],
        });

    playlist.song_requests_enabled = playlist_update.song_requests_enabled;
    playlist.song_arrangements = playlist_update.song_arrangements.to_owned();

    websocket_server_actor_address.do_send(websocket_server_actor::BroadcastAppStateMessage {
        user_id: user_id.to_owned(),
    });

    web::Json(
        state
            .song_requests_by_user_id
            .get(&user_id)
            .unwrap()
            .clone(),
    )
}

#[get("/{user_id}/songs/requests")]
pub async fn list_song_requests_service(
    user_id: web::Path<String>,
    state: web::Data<Mutex<AppState>>,
) -> web::Json<Playlist> {
    let user_id = user_id.into_inner();
    let state = state.lock().unwrap();

    web::Json(
        state
            .song_requests_by_user_id
            .get(&user_id)
            .unwrap_or(&Playlist {
                song_requests_enabled: false,
                song_arrangements: vec![
                    ArrangementType::Lead,
                    ArrangementType::Rhythm,
                    ArrangementType::Bass,
                    ArrangementType::Drums,
                    ArrangementType::Vocals,
                ],
                song_requests: vec![],
            })
            .clone(),
    )
}

#[put("/{user_id}/songs/requests")]
pub async fn create_song_request_service(
    user_id: web::Path<String>,
    song_request: web::Json<SongRequest>,
    app_state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<Playlist> {
    let user_id = user_id.into_inner();
    let song_request = song_request.into_inner();
    let mut state = app_state.lock().unwrap();

    let position = state
        .song_requests_by_user_id
        .get(&user_id)
        .map_or(&vec![], |p| &p.song_requests)
        .iter()
        .position(|id| *id == song_request);

    if let None = position {
        state
            .song_requests_by_user_id
            .entry(user_id.to_owned())
            .or_insert_with(|| Playlist {
                song_requests_enabled: false,
                song_arrangements: vec![
                    ArrangementType::Lead,
                    ArrangementType::Rhythm,
                    ArrangementType::Bass,
                    ArrangementType::Drums,
                    ArrangementType::Vocals,
                ],
                song_requests: vec![],
            })
            .song_requests
            .push(song_request);

        websocket_server_actor_address.do_send(websocket_server_actor::BroadcastAppStateMessage {
            user_id: user_id.to_owned(),
        });
    }

    web::Json(
        state
            .song_requests_by_user_id
            .get(&user_id)
            .unwrap()
            .clone(),
    )
}

#[derive(Deserialize)]
pub struct DeleteSongRequestsQuery {
    index: Option<usize>,
}

#[delete("/{user_id}/songs/requests")]
pub async fn delete_song_requests_service(
    user_id: web::Path<String>,
    query: web::Query<DeleteSongRequestsQuery>,
    state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<Playlist> {
    let user_id = user_id.into_inner();
    let mut state = state.lock().unwrap();
    let position = query.index.unwrap_or(0);

    let song_requests_size = state
        .song_requests_by_user_id
        .get(&user_id)
        .unwrap_or(&Playlist {
            song_requests_enabled: false,
            song_arrangements: vec![
                ArrangementType::Lead,
                ArrangementType::Rhythm,
                ArrangementType::Bass,
                ArrangementType::Drums,
                ArrangementType::Vocals,
            ],
            song_requests: vec![],
        })
        .song_requests
        .len();

    if position < song_requests_size {
        state
            .song_requests_by_user_id
            .get_mut(&user_id)
            .map(|vec| vec.song_requests.remove(position));

        websocket_server_actor_address.do_send(websocket_server_actor::BroadcastAppStateMessage {
            user_id: user_id.to_owned(),
        });
    }

    web::Json(
        state
            .song_requests_by_user_id
            .get(&user_id)
            .unwrap()
            .to_owned(),
    )
}

#[delete("/{user_id}/songs/requests/{song_id}")]
pub async fn delete_song_request_service(
    web::Path((user_id, song_id)): web::Path<(String, String)>,
    state: web::Data<Mutex<AppState>>,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> web::Json<Playlist> {
    let mut state = state.lock().unwrap();

    let position = state
        .song_requests_by_user_id
        .get(&user_id)
        .unwrap_or(&Playlist {
            song_requests_enabled: false,
            song_arrangements: vec![
                ArrangementType::Lead,
                ArrangementType::Rhythm,
                ArrangementType::Bass,
                ArrangementType::Drums,
                ArrangementType::Vocals,
            ],
            song_requests: vec![],
        })
        .song_requests
        .iter()
        .position(|id| *id.song_id == song_id);

    if let Some(position) = position {
        state
            .song_requests_by_user_id
            .get_mut(&user_id)
            .map(|vec| vec.song_requests.remove(position));

        websocket_server_actor_address.do_send(websocket_server_actor::BroadcastAppStateMessage {
            user_id: user_id.to_owned(),
        });
    }

    web::Json(
        state
            .song_requests_by_user_id
            .get(&user_id)
            .unwrap()
            .to_owned(),
    )
}

#[get("/{user_id}/songs/requests/ws")]
pub async fn websocket_service(
    user_id: web::Path<String>,
    request: HttpRequest,
    stream: web::Payload,
    websocket_server_actor_address: web::Data<Addr<websocket_server_actor::WebsocketServerActor>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        WebsocketSessionActor {
            session_id: 0,
            last_heartbeat: Instant::now(),
            room_name: user_id.to_owned(),
            websocket_server_actor_address: websocket_server_actor_address.get_ref().clone(),
        },
        &request,
        stream,
    )
}
