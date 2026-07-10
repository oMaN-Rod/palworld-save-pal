//! Player/guild summary extraction (Task 8). This file is a placeholder so
//! `psp-core` compiles while `SaveSession::load` (Task 7) is implemented
//! ahead of Task 8; the real implementation replaces `extract_summaries`.

use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::session::SaveSession;

pub fn extract_summaries(
    _session: &mut SaveSession,
    _progress: &ProgressSink,
) -> Result<(), CoreError> {
    unimplemented!("Task 8 implements summary extraction")
}
