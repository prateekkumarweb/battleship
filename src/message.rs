use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize)]
// #[serde(tag = "type", content = "data")]
// pub enum Msg {
//     Connect,
//     Id(String),
//     OpponentId(String),
//     Log(String),
//     Chat(String),
//     StartGame(Vec<Ship>),
// }

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy)]
pub struct Ship {
    pub x: u8,
    pub y: u8,
    pub size: u8,
    pub vertical: bool,
    pub sunk: bool,
}

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
