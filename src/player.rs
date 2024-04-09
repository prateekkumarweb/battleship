use std::sync::Arc;

use anyhow::Context;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rand::seq::SliceRandom;
use tokio::{
    sync::{mpsc, oneshot, Mutex, RwLock},
    task::JoinHandle,
};

use crate::message::{PlayerMsg, SystemMsg};

const ID_LENGTH: usize = 8;

#[derive(Debug)]
enum PlayerState {
    Waiting,
    Playing { sender: mpsc::Sender<PlayerMsg> },
}

#[derive(Debug)]
pub struct Player {
    id: String,
    state: Arc<Mutex<PlayerState>>,
    _task: JoinHandle<()>,
    sender: mpsc::Sender<SystemMsg>,
    close_tx: oneshot::Sender<()>,
    is_closed: Arc<RwLock<bool>>,
}

impl Player {
    pub fn new(socket: WebSocket) -> Self {
        let id = generate_random_string();
        let (sender, receiver) = mpsc::channel(1);
        let (close_tx, close_rx) = oneshot::channel();
        let state = Arc::new(Mutex::new(PlayerState::Waiting));
        let is_closed = Arc::new(RwLock::new(false));
        let (ws_tx, ws_rx) = socket.split();
        let actor = PlayerActor {
            ws_tx,
            ws_rx,
            receiver,
            state: state.clone(),
            close_rx,
            is_closed: is_closed.clone(),
        };
        let task = actor.run();

        Self {
            id,
            state,
            _task: task,
            sender,
            close_tx,
            is_closed,
        }
    }

    pub async fn playing(&self, sender: mpsc::Sender<PlayerMsg>) {
        let mut state = self.state.lock().await;
        *state = PlayerState::Playing { sender };
    }

    pub async fn send(&mut self, msg: SystemMsg) -> anyhow::Result<()> {
        self.sender.send(msg).await.context("Send failed")?;
        Ok(())
    }

    pub async fn send_id(&mut self) -> anyhow::Result<()> {
        self.send(SystemMsg::Id(self.id.clone())).await
    }

    pub async fn send_opponent_id(&mut self, opponent: &Self) -> anyhow::Result<()> {
        self.send(SystemMsg::OpponentId(opponent.id.clone())).await
    }

    pub async fn is_closed(&self) -> bool {
        *self.is_closed.read().await
    }

    pub fn close(self) {
        match self.close_tx.send(()) {
            Ok(()) => {}
            Err(()) => {
                tracing::debug!("close_tx already closed");
            }
        }
    }
}

fn generate_random_string() -> String {
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
        .chars()
        .collect();
    let mut rng = rand::thread_rng();
    (0..ID_LENGTH)
        .map(|_| {
            // chars is not empty, so unwrap is safe
            #[allow(clippy::unwrap_used)]
            *chars.choose(&mut rng).unwrap()
        })
        .collect()
}

#[derive(Debug)]
struct PlayerActor {
    ws_tx: SplitSink<WebSocket, Message>,
    ws_rx: SplitStream<WebSocket>,
    receiver: mpsc::Receiver<SystemMsg>,
    state: Arc<Mutex<PlayerState>>,
    close_rx: oneshot::Receiver<()>,
    is_closed: Arc<RwLock<bool>>,
}

impl PlayerActor {
    fn run(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    val = self.ws_rx.next() => {
                        if let Some(Ok(val)) = val {
                            if let Err(e) = self.handle_ws_msg(val).await {
                                tracing::error!("Failed to handle ws message: {:?}", e);
                                break;
                            }
                        } else {
                            tracing::debug!("Received None");
                            break;
                        }

                    }
                    val = self.receiver.recv() => {
                        if let Some(msg) = val {
                            let Ok(msg_json) = serde_json::to_string(&msg) else {
                                tracing::error!("Failed to serialize message {:?}", msg);
                                break;
                            };
                            if let Err(e) =
                            self.ws_tx.send(Message::Text(msg_json)).await {
                                tracing::error!("Failed to send message: {:?}", e);
                                break;
                            }
                        } else {
                            if let Err(e) = self.ws_tx.close().await {
                                tracing::error!("Failed to close: {:?}", e);
                            }
                            break;
                        }
                    }
                    val = &mut self.close_rx => {
                        match val {
                            Ok(()) => {
                                if let Err(e) = self.ws_tx.close().await {
                                    tracing::error!("Failed to close: {:?}", e);
                                }
                            }
                            Err(_) => {
                                tracing::debug!("close_rx already closed");
                            }
                        }
                        break;
                    }
                };
            }
            *self.is_closed.write().await = true;
            match &mut *self.state.lock().await {
                PlayerState::Playing { ref sender } => {
                    if let Err(e) = sender.send(PlayerMsg::Close).await {
                        tracing::error!("Failed to send close: {:?}", e);
                    }
                }
                PlayerState::Waiting => {}
            }
        })
    }

    async fn handle_ws_msg(&mut self, val: Message) -> anyhow::Result<()> {
        if let Ok(val) = val.to_text() {
            if val.is_empty() {
                return Ok(());
            }
            tracing::debug!("Received message: {:?}", &val);
            let msg: PlayerMsg = serde_json::from_str(val)?;
            let state = self.state.lock().await;
            if let PlayerState::Playing { ref sender } = *state {
                sender.send(msg).await?;
            } else {
                tracing::debug!("Received message on waiting: {:?}", &msg);
            }
        } else {
            tracing::debug!("Received invalid message: {:?}", &val);
            return Ok(());
        }
        Ok(())
    }
}
