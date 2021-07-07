use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::*;
use derive_more::{Display, Error};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::path::PathBuf;

struct AppState {
    requested_song_ids: Mutex<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddSongRequestRequest {
    song_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSongRequestRequest {
    song_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSongRequestsResponse {
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
async fn index() -> Result<NamedFile> {
    let path: PathBuf = "songs.json".parse().unwrap();
    Ok(NamedFile::open(path)?)
}

#[get("/song-request")]
async fn list_song_requests(
    state: web::Data<AppState>
) -> Result<web::Json<ListSongRequestsResponse>> {
    let guard = state.requested_song_ids.lock().unwrap();
    let requested_song_ids = (*guard).clone();
    Ok(web::Json(ListSongRequestsResponse { requested_song_ids }))
}

#[post("/song-request")]
async fn add_song_request(
    add_song_request: web::Json<AddSongRequestRequest>,
    state: web::Data<AppState>
) -> Result<web::Json<ListSongRequestsResponse>, SongRequestError> {
    let mut guard = state.requested_song_ids.lock().unwrap();

    return if let None = (*guard).iter().position(|id| *id == add_song_request.song_id) {
        (*guard).push(add_song_request.song_id.clone());
        Ok(web::Json(ListSongRequestsResponse { requested_song_ids: (*guard).clone() }))
    } else {
        Err(SongRequestError { description: "Existing song id." })
    }
}

#[delete("/song-request")]
async fn delete_song_request(
    delete_song_request: web::Json<DeleteSongRequestRequest>,
    state: web::Data<AppState>
) -> Result<web::Json<ListSongRequestsResponse>, SongRequestError> {
    let mut guard = state.requested_song_ids.lock().unwrap();

    return if let Some(position) = (*guard).iter().position(|id| *id == delete_song_request.song_id) {
        (*guard).remove(position);
        Ok(web::Json(ListSongRequestsResponse { requested_song_ids: (*guard).clone() }))
    } else {
        Err(SongRequestError { description: "Invalid song id." })
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = web::Data::new(AppState {
        requested_song_ids: Mutex::new(vec![]),
    });

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(state.clone())
            .service(index)
            .service(list_song_requests)
            .service(add_song_request)
            .service(delete_song_request)
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
