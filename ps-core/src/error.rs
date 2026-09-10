/// Domain-level errors. uesave errors are stringified into `Parse` at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("no save loaded")]
    SaveNotLoaded,
    #[error("player not found: {0}")]
    PlayerNotFound(uuid::Uuid),
    #[error("pal not found: {0}")]
    PalNotFound(uuid::Uuid),
    #[error("guild not found: {0}")]
    GuildNotFound(uuid::Uuid),
    #[error("parse error: {0}")]
    Parse(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::CoreError;

    #[test]
    fn display_strings_match_contract() {
        assert_eq!(CoreError::SaveNotLoaded.to_string(), "no save loaded");
        assert_eq!(
            CoreError::Parse("bad magic".into()).to_string(),
            "parse error: bad magic"
        );
        assert_eq!(CoreError::Other("boom".into()).to_string(), "boom");
    }
}
