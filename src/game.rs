use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    message::{PlayerMsg, SystemMsg},
    player::Player,
};

#[derive(Debug)]
pub struct Game {
    #[allow(dead_code)]
    task: JoinHandle<()>,
}

impl Game {
    pub async fn new(player1: Player, player2: Player) -> Self {
        let (tx1, rx1) = mpsc::channel(1);
        let (tx2, rx2) = mpsc::channel(1);
        tokio::join!(player1.playing(tx1), player2.playing(tx2));
        let actor = GameActor {
            players: [player1, player2],
            rxs: (rx1, rx2),
            stage: GameStage::Init,
            player_state: [PlayerGameState::default(), PlayerGameState::default()],
        };
        let task = actor.run();
        Self { task }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy)]
pub struct Ship {
    pub x: u8,
    pub y: u8,
    pub size: u8,
    pub vertical: bool,
    pub sunk: bool,
}
struct GameActor {
    players: [Player; 2],
    rxs: (mpsc::Receiver<PlayerMsg>, mpsc::Receiver<PlayerMsg>),
    stage: GameStage,
    player_state: [PlayerGameState; 2],
}

#[derive(Debug, Clone, Copy)]
enum GameStage {
    Init,
    Turn(PlayerIndex),
    Over,
}

#[derive(Debug, Default, Clone, Copy)]
struct BoardCell {
    ship: Option<usize>,
    striked: bool,
}

#[derive(Debug, Default)]
struct PlayerGameState {
    ships_placed: bool,
    ships: [Ship; 5],
    board: [[BoardCell; 10]; 10],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PlayerIndex {
    P0,
    P1,
}

impl PlayerIndex {
    const fn index(self) -> usize {
        match self {
            Self::P0 => 0,
            Self::P1 => 1,
        }
    }
    const fn opponent(self) -> Self {
        match self {
            Self::P0 => Self::P1,
            Self::P1 => Self::P0,
        }
    }
}

fn validate_ships(ships: &[Ship; 5]) -> Option<[[BoardCell; 10]; 10]> {
    let mut board = [[BoardCell::default(); 10]; 10];
    if ships[0].size != 5 {
        return None;
    }
    if ships[1].size != 4 {
        return None;
    }
    if ships[2].size != 3 {
        return None;
    }
    if ships[3].size != 3 {
        return None;
    }
    if ships[4].size != 2 {
        return None;
    }
    for (ship_index, ship) in ships.iter().enumerate() {
        if ship.x >= 10 || ship.y >= 10 {
            return None;
        }
        if ship.vertical {
            if ship.y + ship.size > 10 {
                return None;
            }
            for i in 0..ship.size {
                if board[(ship.y + i) as usize][ship.x as usize].ship.is_some() {
                    return None;
                }
                board[(ship.y + i) as usize][ship.x as usize].ship = Some(ship_index);
            }
        } else {
            if ship.x + ship.size > 10 {
                return None;
            }
            for i in 0..ship.size {
                if board[ship.y as usize][(ship.x + i) as usize].ship.is_some() {
                    return None;
                }
                board[ship.y as usize][(ship.x + i) as usize].ship = Some(ship_index);
            }
        }
    }
    Some(board)
}

impl GameActor {
    fn run(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = self.rxs.0.recv() => {
                        if let Some(msg) = msg {
                            tracing::debug!("Received message: {:?}", &msg);
                            self.handle(msg, PlayerIndex::P0).await;
                        } else {
                            if let Err(e) = self.players[1].send(SystemMsg::YouWin).await {
                                tracing::error!("Error sending you win message: {:?}", e);
                            }
                            self.stage = GameStage::Over;
                            break;
                        }
                    }
                    msg = self.rxs.1.recv() => {
                        if let Some(msg) = msg {
                            tracing::debug!("Received message: {:?}", &msg);
                            self.handle(msg, PlayerIndex::P1).await;
                        } else {
                            if let Err(e) = self.players[1].send(SystemMsg::YouWin).await {
                                tracing::error!("Error sending you win message: {:?}", e);
                            }
                            self.stage = GameStage::Over;
                            break;
                        }
                    }
                }
            }
        })
    }

    async fn handle_move(&mut self, p: PlayerIndex, x: u8, y: u8) -> anyhow::Result<()> {
        if x >= 10 || y >= 10 {
            self.players[p.index()].send(SystemMsg::InvalidMove).await?;
        } else {
            let cell = &mut self.player_state[p.opponent().index()].board[y as usize][x as usize];
            if cell.striked {
                self.players[p.index()].send(SystemMsg::AlreadyHit).await?;
            } else {
                cell.striked = true;
                if let Some(ship_index) = cell.ship {
                    let ship = &mut self.player_state[p.opponent().index()].ships[ship_index];
                    if ship.sunk {
                        self.players[p.index()].send(SystemMsg::InvalidMove).await?;
                    } else {
                        self.players[p.index()].send(SystemMsg::Hit(x, y)).await?;
                        self.players[p.opponent().index()]
                            .send(SystemMsg::OppHit(x, y))
                            .await?;
                        let mut sunk = true;
                        for i in 0..ship.size {
                            let cell = if ship.vertical {
                                self.player_state[p.opponent().index()].board[(ship.y + i) as usize]
                                    [ship.x as usize]
                            } else {
                                self.player_state[p.opponent().index()].board[ship.y as usize]
                                    [(ship.x + i) as usize]
                            };
                            if !cell.striked {
                                sunk = false;
                            }
                        }
                        if sunk {
                            ship.sunk = sunk;
                            self.players[p.index()]
                                .send(SystemMsg::Sank {
                                    index: ship_index,
                                    ship: *ship,
                                })
                                .await?;
                        }
                        if self.player_state[p.opponent().index()]
                            .ships
                            .iter()
                            .all(|s| s.sunk)
                        {
                            self.players[p.index()].send(SystemMsg::YouWin).await?;
                            self.players[p.opponent().index()]
                                .send(SystemMsg::OppWin)
                                .await?;
                            self.stage = GameStage::Over;
                        }
                    }
                } else {
                    self.players[p.index()].send(SystemMsg::Miss(x, y)).await?;
                    self.players[p.opponent().index()]
                        .send(SystemMsg::OppMiss(x, y))
                        .await?;
                    self.stage = GameStage::Turn(p.opponent());
                    self.players[p.opponent().index()]
                        .send(SystemMsg::YourTurn)
                        .await?;
                    self.players[p.index()].send(SystemMsg::OppTurn).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle(&mut self, msg: PlayerMsg, p: PlayerIndex) {
        match (self.stage, msg) {
            (_, PlayerMsg::Connect) => {}
            (_, PlayerMsg::Close) => {
                if let Err(e) = self.players[p.opponent().index()]
                    .send(SystemMsg::Close)
                    .await
                {
                    tracing::error!("Error sending close message: {:?}", e);
                }
            }
            (_, PlayerMsg::Chat(msg)) => {
                if let Err(e) = self.players[p.opponent().index()]
                    .send(SystemMsg::Chat(msg))
                    .await
                {
                    tracing::error!("Error sending chat message: {:?}", e);
                }
            }
            (GameStage::Init, PlayerMsg::StartGame(ships)) => {
                if let Err(e) = self.start_game(p, ships).await {
                    tracing::error!("Error starting game: {:?}", e);
                }
            }
            (GameStage::Init, PlayerMsg::Move(_, _)) => {
                if let Err(e) = self.players[p.index()]
                    .send(SystemMsg::Log("Game has not started yet".to_owned()))
                    .await
                {
                    tracing::error!("Error sending log message: {:?}", e);
                }
            }
            (GameStage::Turn(_), PlayerMsg::StartGame(_)) => {
                if let Err(e) = self.players[p.index()]
                    .send(SystemMsg::Log("Game has already started".to_owned()))
                    .await
                {
                    tracing::error!("Error sending log message: {:?}", e);
                }
            }
            (GameStage::Turn(turn), PlayerMsg::Move(x, y)) => {
                if turn == p {
                    if let Err(e) = self.handle_move(p, x, y).await {
                        tracing::error!("Error handling move: {:?}", e);
                    }
                } else if let Err(e) = self.players[p.index()].send(SystemMsg::NotYourTurn).await {
                    tracing::error!("Error sending not your turn message: {:?}", e);
                }
            }
            (GameStage::Over, _) => {
                if let Err(e) = self.players[p.index()]
                    .send(SystemMsg::Log("Game Over".to_owned()))
                    .await
                {
                    tracing::error!("Error sending log message: {:?}", e);
                }
            }
        }
    }

    async fn start_game(&mut self, p: PlayerIndex, ships: [Ship; 5]) -> anyhow::Result<()> {
        if let Some(board) = validate_ships(&ships) {
            self.player_state[p.index()].board = board;
            self.player_state[p.index()].ships = ships;
            self.player_state[p.index()].ships_placed = true;
            if self.player_state[0].ships_placed && self.player_state[1].ships_placed {
                self.players[0].send(SystemMsg::StartGame).await?;
                self.players[1].send(SystemMsg::StartGame).await?;
                self.stage = GameStage::Turn(PlayerIndex::P0);
                self.players[0].send(SystemMsg::YourTurn).await?;
                self.players[1].send(SystemMsg::OppTurn).await?;
            }
            self.players[p.index()]
                .send(SystemMsg::Log("Ships Placed".to_owned()))
                .await?;
            self.players[p.opponent().index()]
                .send(SystemMsg::Log("Opponent Placed Ships".to_owned()))
                .await?;
        } else {
            self.players[p.index()]
                .send(SystemMsg::Log("Invalid Ships".to_owned()))
                .await?;
        }
        Ok(())
    }
}
