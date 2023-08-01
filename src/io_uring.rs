#![cfg(target_os = "linux")]

use blake3::Hasher;
use crossbeam::channel::{self, Receiver};
use io_uring::{opcode, IoUring};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::thread;
use walkdir::{DirEntry, WalkDir};

use crate::types::{FileInfo, Inputs, Outputs};

type Data = DirEntry;

pub fn main(inputs: Inputs) -> Outputs {
    let Inputs {
        directory,
        threads,
        max_fds: _,
        buffer_size,
        io_uring_ring_size: _,
    } = inputs;

    let (sender, receiver) = channel::unbounded::<Data>();
    let mut workers = Vec::with_capacity(threads);

    let mut all_files = Vec::new();

    let start = std::time::Instant::now();

    for _ in 0..threads {
        let receiver = receiver.clone();
        let worker = thread::spawn(move || start_worker(buffer_size, receiver));
        workers.push(worker);
    }

    for entry in WalkDir::new(directory) {
        let entry = entry.unwrap();

        let metadata = entry.metadata().unwrap();
        let ft = metadata.file_type();

        if ft.is_file() {
            sender.send(entry).unwrap();
        } else {
            //
        }
    }

    // Close the write side of the channel.
    drop(sender);

    for worker in workers {
        all_files.extend(worker.join().unwrap());
    }

    Outputs {
        start,
        files: all_files,
    }
}

fn start_worker(buffer_size: usize, receiver: Receiver<Data>) -> Vec<FileInfo> {
    let mut ring = IoUring::new(256).unwrap();

    let mut hasher = Hasher::new();
    let mut buffer = vec![0u8; buffer_size];
    let mut offset;

    let mut all_results = Vec::new();
    while let Ok(entry) = receiver.recv() {
        let file = File::open(entry.path()).unwrap();
        let fd = file.as_raw_fd();

        let len = file.metadata().unwrap().len();

        hasher.reset();

        offset = 0;

        while offset < len {
            let read_e = opcode::Read::new(
                io_uring::types::Fd(fd),
                buffer.as_mut_ptr() as _,
                buffer.len() as _,
            )
            .offset(offset)
            .build();

            unsafe {
                ring.submission().push(&read_e).unwrap();
            }
            ring.submit_and_wait(1).unwrap();

            let cqe = ring.completion().next().unwrap();
            let bytes_read = cqe.result() as usize;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);

            offset += bytes_read as u64;
        }

        let hash: [u8; 32] = hasher.finalize().into();

        all_results.push(FileInfo { hash });
    }

    all_results
}
