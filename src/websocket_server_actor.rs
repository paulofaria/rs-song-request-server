use actix::prelude::*;
use rand::{self, rngs::ThreadRng, Rng};

use std::sync::{Mutex};

use std::collections::{HashMap, HashSet};
use crate::{AppState, SongRequest, Playlist};
use actix_web::web::Data;

use serde::{Serialize};
use crate::websocket_session_actor::{WebsocketReplyMessage};

pub struct WebsocketServerActor {
    recipients_by_session_id: HashMap<usize, Recipient<WebsocketReplyMessage>>,
    session_ids_by_room_name: HashMap<String, HashSet<usize>>,
    random_number_generator: ThreadRng,
    app_state: Data<Mutex<AppState>>,
}

impl Actor for WebsocketServerActor {
    type Context = Context<Self>;
}

impl WebsocketServerActor {
    pub fn new(state: Data<Mutex<AppState>>) -> WebsocketServerActor {
        WebsocketServerActor {
            recipients_by_session_id: HashMap::new(),
            session_ids_by_room_name: HashMap::new(),
            random_number_generator: rand::thread_rng(),
            app_state: state,
        }
    }
}

impl WebsocketServerActor {
    /// Send message to all client sessions in the room.
    fn send_message(&self, room_name: &str, message: &str, skip_session_id: usize) {
        if let Some(session_ids) = self.session_ids_by_room_name.get(room_name) {
            for session_id in session_ids {
                if *session_id != skip_session_id {
                    if let Some(reply_message_recipient) = self.recipients_by_session_id.get(session_id) {
                        reply_message_recipient.do_send(
                            WebsocketReplyMessage { message: message.to_owned() }
                        ).unwrap();
                    }
                }
            }
        }
    }
}

/// New chat session is created
#[derive(Message)]
#[rtype(usize)]
pub struct ConnectMessage {
    pub room_name: String,
    pub websocket_session_actor_recipient: Recipient<WebsocketReplyMessage>,
}

/// Register new session and assign unique id to this session.
impl Handler<ConnectMessage> for WebsocketServerActor {
    type Result = usize;

    fn handle(&mut self, connect_message: ConnectMessage, _: &mut Context<Self>) -> Self::Result {
        // Notify all users in the same room.
        // self.send_message(&MAIN_ROOM.to_owned(), "Someone joined", 0);

        // Register session with random id.
        let session_id = self.random_number_generator.gen::<usize>();
        self.recipients_by_session_id.insert(session_id, connect_message.websocket_session_actor_recipient);

        // Auto join room.
        self.session_ids_by_room_name
            .entry(connect_message.room_name.to_owned())
            .or_insert_with(HashSet::new)
            .insert(session_id);

        log::debug!("Client with session id '{}' connected.", session_id);
        // Return client session id back.
        session_id
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct DisconnectMessage {
    pub websocket_session_id: usize,
}

impl Handler<DisconnectMessage> for WebsocketServerActor {
    type Result = ();

    fn handle(&mut self, disconnect_message: DisconnectMessage, _: &mut Context<Self>) {
        let mut rooms: Vec<String> = Vec::new();

        // Remove client session.
        if self.recipients_by_session_id.remove(&disconnect_message.websocket_session_id).is_some() {
            // Remove session from all rooms.
            for (room_name, sessions) in &mut self.session_ids_by_room_name {
                if sessions.remove(&disconnect_message.websocket_session_id) {
                    rooms.push(room_name.to_owned());
                }
            }
        }
        // // send message to other users
        // for room in rooms {
        //     self.send_message(&room, "Someone disconnected", 0);
        // }

        log::debug!("Client with session id '{}' disconnected.", disconnect_message.websocket_session_id);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    /// Id of the client session
    pub session_id: usize,
    /// Room name
    pub room_name: String,
    /// Peer message
    pub message: String,
}

impl Handler<ClientMessage> for WebsocketServerActor {
    type Result = ();

    fn handle(&mut self, client_message: ClientMessage, _: &mut Context<Self>) {
        self.send_message(&client_message.room_name, client_message.message.as_str(), client_message.session_id);
    }
}

pub struct ListRoomsMessage;

impl actix::Message for ListRoomsMessage {
    type Result = Vec<String>;
}

/// Handler for `ListRooms` message.
impl Handler<ListRoomsMessage> for WebsocketServerActor {
    type Result = MessageResult<ListRoomsMessage>;

    fn handle(&mut self, _: ListRoomsMessage, _: &mut Context<Self>) -> Self::Result {
        let mut room_names = Vec::new();

        for room_name in self.session_ids_by_room_name.keys() {
            room_names.push(room_name.to_owned())
        }

        MessageResult(room_names)
    }
}

/// Join room, if room does not exists create new one.
#[derive(Message)]
#[rtype(result = "()")]
pub struct JoinMessage {
    /// Client session id.
    pub session_id: usize,
    /// Room name.
    pub room_name: String,
}

/// Join room, send disconnect message to old room
/// send join message to new room
impl Handler<JoinMessage> for WebsocketServerActor {
    type Result = ();

    fn handle(&mut self, join_message: JoinMessage, _: &mut Context<Self>) {
        let JoinMessage { session_id, room_name } = join_message;
        let mut room_names = Vec::new();

        // Remove session from all rooms.
        for (room_name, session_ids) in &mut self.session_ids_by_room_name {
            if session_ids.remove(&session_id) {
                room_names.push(room_name.to_owned());
            }
        }

        // Send message to other users.
        for room_name in room_names {
            self.send_message(&room_name, "Someone disconnected", 0);
        }

        self.session_ids_by_room_name
            .entry(room_name.clone())
            .or_insert_with(HashSet::new)
            .insert(session_id);

        self.send_message(&room_name, "Someone connected", session_id);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastAppStateMessage {
    pub user_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStateResponse {
    song_requests_enabled: bool,
    song_requests: Vec<SongRequest>,
}

impl Handler<BroadcastAppStateMessage> for WebsocketServerActor {
    type Result = ();

    fn handle(&mut self, broadcast_app_state_message: BroadcastAppStateMessage, _: &mut Context<Self>) {
        let app_state = self.app_state.lock().unwrap();

        let default_playlist = Playlist {
            song_requests_enabled: false,
            song_requests: vec![]
        };

        let playlist = app_state.song_requests_by_user_id
            .get(&broadcast_app_state_message.user_id)
            .unwrap_or(&default_playlist);

        let serialized_app_state_response = serde_json::to_string(&AppStateResponse {
            song_requests_enabled: playlist.song_requests_enabled,
            song_requests: playlist.song_requests.clone(),
        }).unwrap();

        log::debug!("Broadcasted app state: {:?}", serialized_app_state_response);
        self.send_message(&broadcast_app_state_message.user_id, serialized_app_state_response.as_str(), 0);
    }
}