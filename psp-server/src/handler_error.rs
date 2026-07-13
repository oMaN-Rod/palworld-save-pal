/// Any handler failure. The dispatcher converts these into the `error`
/// WS message ({message, trace}); the connection always survives.
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("{0}")]
    Core(#[from] psp_core::error::CoreError),
    #[error("{0}")]
    Db(#[from] psp_db::error::DbError),
    #[error("invalid payload: {0}")]
    Payload(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::HandlerError;
    use psp_core::error::CoreError;

    /// The `#[error(...)]` strings end up in the `message` field of the wire
    /// `error` frame the frontend renders, so pin them.
    #[test]
    fn display_strings_match_contract() {
        assert_eq!(
            HandlerError::Core(CoreError::SaveNotLoaded).to_string(),
            "no save loaded"
        );
        let payload_error = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        assert!(HandlerError::Payload(payload_error)
            .to_string()
            .starts_with("invalid payload: "));
        assert_eq!(HandlerError::Other("boom".into()).to_string(), "boom");
    }
}
