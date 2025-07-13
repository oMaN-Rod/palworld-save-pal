use crate::models::player::Player;
use std::sync::{Arc, Mutex};

// We need to derive Clone so we can share this state
#[derive(Debug, Default, Clone)]
pub struct AppStateInner {
    pub players: Option<Vec<Player>>,
}

// Change the state to use an Arc (Atomic Reference Counter) for safe multi-threaded sharing and cloning.
#[derive(Debug, Clone, Default)]
pub struct AppState(pub Arc<Mutex<AppStateInner>>);
