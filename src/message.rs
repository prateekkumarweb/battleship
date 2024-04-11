use serde::{Deserialize, Serialize};

use crate::game::Ship;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PlayerMsg {
    Connect,
    Chat(String),
    StartGame([Ship; 5]),
    Move(u8, u8),
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SystemMsg {
    Log(String),
    Id(String),
    OpponentId(String),
    Chat(String),
    StartGame,
    NotYourTurn,
    InvalidMove,
    AlreadyHit,
    YourTurn,
    OppTurn,
    Hit(u8, u8),
    Miss(u8, u8),
    OppHit(u8, u8),
    OppMiss(u8, u8),
    Sank { index: usize, ship: Ship },
    YouWin,
    OppWin,
    Close,
}
