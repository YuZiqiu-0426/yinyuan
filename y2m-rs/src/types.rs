#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionLoopExit {
    UserQuit,
    Disconnected,
}

pub(crate) struct FileAcceptInfo {
    pub(crate) save_path: Option<String>,
}
