use super::pal::Pal;
use crate::graphql::ID;
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "Player")]
pub struct Player {
    pub uid: ID,
    pub nickname: String,
    pub level: u32,
    pub pals: Vec<Pal>,
}
