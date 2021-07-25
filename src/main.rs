use actix::*;
use actix_web::*;
use actix_cors::Cors;
use std::sync::Mutex;
use std::env;

use crate::http_routes::{list_songs, delete_song_requests_service, update_playlist};
use crate::http_routes::list_song_requests_service;
use crate::http_routes::create_song_request_service;
use crate::http_routes::delete_song_request_service;
use crate::http_routes::websocket_service;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

mod http_routes;
mod websocket_server_actor;
mod websocket_session_actor;

pub struct AppState {
    song_requests_by_user_id: HashMap<String, Playlist>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    song_requests_enabled: bool,
    song_requests: Vec<SongRequest>,
}

#[derive(Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SongRequest {
    viewer_id: String,
    viewer_username: String,
    song_id: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    let app_state = web::Data::new(Mutex::new(AppState {
        song_requests_by_user_id: HashMap::new(),
    }));

    let websocket_server_actor_address = websocket_server_actor::WebsocketServerActor::new(app_state.clone()).start();

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .data(websocket_server_actor_address.clone())
            .service(list_songs)
            .service(update_playlist)
            .service(list_song_requests_service)
            .service(create_song_request_service)
            .service(delete_song_requests_service)
            .service(delete_song_request_service)
            .service(websocket_service)
    })
        .bind(("0.0.0.0", port))?
        .run()
        .await
}
