use crate::graphql::ID; // <-- Import the new ID type
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(name = "Pal")]
pub struct Pal {
    pub instance_id: ID,
    pub character_id: String,
    pub nickname: Option<String>,
    pub level: u32,
}
