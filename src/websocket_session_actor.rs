use std::time::{Duration, Instant};

use actix::*;
use actix_web_actors::ws;
use crate::websocket_server_actor;

pub struct WebsocketSessionActor {
    /// Unique client session id.
    pub session_id: usize,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop the connection.
    pub last_heartbeat: Instant,
    /// Room the client session is in.
    pub room_name: String,
    /// Websocket server actor address.
    pub websocket_server_actor_address: Addr<websocket_server_actor::WebsocketServerActor>,
}

impl Actor for WebsocketSessionActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, websocket_context: &mut Self::Context) {
        self.check_heartbeat(websocket_context);
        let websocket_session_actor_address = websocket_context.address();

        self.websocket_server_actor_address
            .send(websocket_server_actor::ConnectMessage {
                room_name: self.room_name.to_owned(),
                websocket_session_actor_recipient: websocket_session_actor_address.recipient(),
            })
            .into_actor(self)
            .then(|result, websocket_session_actor, websocket_context| {
                match result {
                    Ok(session_id) => websocket_session_actor.session_id = session_id,
                    _ => websocket_context.stop(),
                }

                fut::ready(())
            })
            .wait(websocket_context);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.websocket_server_actor_address.do_send(
            websocket_server_actor::DisconnectMessage { websocket_session_id: self.session_id }
        );

        Running::Stop
    }
}

/// How often heartbeat pings are sent.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

impl WebsocketSessionActor {
    fn check_heartbeat(&self, context: &mut ws::WebsocketContext<Self>) {
        context.run_interval(HEARTBEAT_INTERVAL, |websocket_session_actor, websocket_context| {
            if Instant::now().duration_since(websocket_session_actor.last_heartbeat) > CLIENT_TIMEOUT {
                log::debug!("Client session heartbeat failed, disconnecting!");

                websocket_session_actor.websocket_server_actor_address.do_send(
                    websocket_server_actor::DisconnectMessage {
                        websocket_session_id: websocket_session_actor.session_id
                    }
                );

                return websocket_context.stop();
            }

            log::debug!("Sent ping message to client with session id {}.", websocket_session_actor.session_id);
            websocket_context.ping(b"");
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebsocketSessionActor {
    fn handle(
        &mut self,
        websocket_message: Result<ws::Message, ws::ProtocolError>,
        websocket_context: &mut Self::Context,
    ) {
        let websocket_message = match websocket_message {
            Ok(websocket_message) => websocket_message,
            Err(_) => {
                return websocket_context.stop();
            }
        };

        match websocket_message {
            ws::Message::Ping(ping_message) => {
                log::debug!("Received ping message from client with session id {}.", self.session_id);
                self.last_heartbeat = Instant::now();
                websocket_context.pong(&ping_message);
            }
            ws::Message::Pong(_) => {
                log::debug!("Received pong message from client with session id {}.", self.session_id);
                self.last_heartbeat = Instant::now();
            }
            ws::Message::Text(text_message) => {
                log::debug!("Received text message from client with session id {}: {}", self.session_id, text_message);
                let trimmed_message = text_message.trim();

                if trimmed_message.starts_with('/') {
                    let words: Vec<&str> = trimmed_message
                        .splitn(2, ' ')
                        .collect();

                    match words[0] {
                        "/list" => {
                            log::debug!("Received /list message");

                            self.websocket_server_actor_address
                                .send(websocket_server_actor::ListRoomsMessage)
                                .into_actor(self)
                                .then(|result, _, websocket_context| {
                                    match result {
                                        Ok(rooms) => {
                                            for room in rooms {
                                                websocket_context.text(room);
                                            }
                                        }
                                        _ => log::error!("Websocket server actor failed to respond to /list command."),
                                    }
                                    fut::ready(())
                                })
                                .wait(websocket_context)
                        }
                        "/join" => {
                            log::debug!("Received /join message");

                            if words.len() == 2 {
                                self.room_name = words[1].to_owned();

                                self.websocket_server_actor_address.do_send(websocket_server_actor::JoinMessage {
                                    session_id: self.session_id,
                                    room_name: self.room_name.clone(),
                                });

                                websocket_context.text("joined");
                            } else {
                                websocket_context.text("!!! room name is required");
                            }
                        }
                        _ => websocket_context.text(format!("!!! unknown command: {:?}", trimmed_message)),
                    }
                } else {
                    self.websocket_server_actor_address.do_send(websocket_server_actor::ClientMessage {
                        session_id: self.session_id,
                        message: trimmed_message.to_owned(),
                        room_name: self.room_name.clone(),
                    })
                }
            }
            ws::Message::Binary(_) => log::error!("Unexpected binary websocket message."),
            ws::Message::Close(close_reason) => {
                if let Some(close_reason) = &close_reason {
                    log::debug!("Received close message from client with session id {}: {:?}", self.session_id, close_reason);
                } else {
                    log::debug!("Received close message from client with session id {}.", self.session_id);
                }
                websocket_context.close(close_reason);
                websocket_context.stop();
            }
            ws::Message::Continuation(_) => {
                log::debug!("Received continuation message from client with session id {}.", self.session_id);
                websocket_context.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WebsocketReplyMessage {
    pub message: String,
}

impl Handler<WebsocketReplyMessage> for WebsocketSessionActor {
    type Result = ();

    fn handle(&mut self, websocket_reply_message: WebsocketReplyMessage, websocket_context: &mut Self::Context) {
        websocket_context.text(websocket_reply_message.message);
    }
}
