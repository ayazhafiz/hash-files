use std::path::PathBuf;

pub type HashOutcome = [u8; 32];

#[derive(Debug)]
pub struct FileInfo {
    pub hash: HashOutcome,
}

pub struct Inputs {
    pub directory: PathBuf,
    pub threads: usize,
    pub max_fds: usize,
    pub buffer_size: usize,
    pub io_uring_ring_size: usize,
}

pub struct Outputs {
    pub start: std::time::Instant,
    pub files: Vec<FileInfo>,
}
