use enigo::Key;
use serde::Deserialize;

use crate::cube::Move;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub timeout: u64,
    pub binds: Vec<Bind>,
}

#[derive(Deserialize, Debug)]
pub struct Bind {
    pub trigger: Vec<Move>,
    pub actions: Vec<Action>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum Action {
    Press(Key),
    Release(Key),
    Click(Key),
}
