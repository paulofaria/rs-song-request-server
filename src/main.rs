use actix::*;
use actix_web::*;
use actix_cors::Cors;
use std::sync::Mutex;
use std::env;

use crate::http_routes::index_service;
use crate::http_routes::list_song_requests_service;
use crate::http_routes::create_song_request_service;
use crate::http_routes::delete_song_request_service;
use crate::http_routes::websocket_service;

mod http_routes;
mod websocket_server_actor;
mod websocket_session_actor;

pub struct AppState {
    requested_song_ids: Vec<String>,
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
