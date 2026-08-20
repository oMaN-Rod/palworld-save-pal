#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    Ok,
    Timeout,
    Cancelled,
    MemoryExceeded,
    Error(String),
}

impl RunStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, RunStatus::Ok)
    }

    pub fn as_wire(&self) -> &'static str {
        match self {
            RunStatus::Ok => "ok",
            RunStatus::Timeout => "timeout",
            RunStatus::Cancelled => "cancelled",
            RunStatus::MemoryExceeded => "memory_exceeded",
            RunStatus::Error(_) => "error",
        }
    }
}
