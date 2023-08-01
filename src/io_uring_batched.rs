#![cfg(target_os = "linux")]

use blake3::Hasher;
use crossbeam::channel::{self, Receiver};
use io_uring::{opcode, IoUring};
use std::fs::Metadata;
use std::os::fd::{FromRawFd, IntoRawFd};

use std::fs::File;
use std::thread;
use walkdir::{DirEntry, WalkDir};

use crate::types::{FileInfo, Inputs, Outputs};

type Data = (DirEntry, Metadata);

pub fn main(inputs: Inputs) -> Outputs {
    let Inputs {
        directory,
        threads,
        max_fds: _,
        buffer_size,
        io_uring_ring_size,
    } = inputs;

    let buffer_size_u32 = buffer_size as u32;
    let io_uring_ring_size_u32 = io_uring_ring_size as u32;

    let (sender, receiver) = channel::unbounded::<Data>();
    let mut workers = Vec::with_capacity(threads);
    let mut all_files = Vec::new();

    let mut rings = std::iter::repeat_with(|| IoUring::new(io_uring_ring_size_u32).unwrap())
        .take(threads)
        .collect::<Vec<_>>();

    let mut hashing_states_for_threads =
        std::iter::repeat_with(|| HashingStateMap::new(io_uring_ring_size))
            .take(threads)
            .collect::<Vec<_>>();

    let mut reuse_buffers_for_threads =
        std::iter::repeat_with(|| ReuseBuffers::new(io_uring_ring_size, buffer_size))
            .take(threads)
            .collect::<Vec<_>>();

    let start = std::time::Instant::now();

    for _ in 0..threads {
        let receiver = receiver.clone();
        let ring = rings.pop().unwrap();
        let hashing_states = hashing_states_for_threads.pop().unwrap();
        let reuse_buffers = reuse_buffers_for_threads.pop().unwrap();
        let worker = thread::spawn(move || {
            start_workers(WorkerState {
                ring,
                hashing_states,
                reuse_buffers,
                buffer_size: buffer_size_u32,
                ring_size: io_uring_ring_size,
                receiver,
            })
        });
        workers.push(worker);
    }

    for entry in WalkDir::new(directory).follow_links(true) {
        let entry = entry.unwrap();

        let metadata = entry.metadata().unwrap();
        let ft = metadata.file_type();

        if ft.is_file() {
            sender.send((entry, metadata)).unwrap();
        } else {
            //
        }
    }

    drop(sender); // signal the workers that no more files will be sent

    for worker in workers {
        all_files.extend(worker.join().unwrap());
    }

    Outputs {
        start,
        files: all_files,
    }
}

type Buffer = Box<[u8]>;

struct HashingState {
    hasher: Hasher,
    buffer: Buffer,
    offset: u64,
    len: u64,
}

struct WorkerState {
    ring: IoUring,
    hashing_states: HashingStateMap,
    reuse_buffers: ReuseBuffers,
    buffer_size: u32,
    ring_size: usize,
    receiver: Receiver<Data>,
}

fn start_workers(state: WorkerState) -> Vec<FileInfo> {
    let WorkerState {
        mut ring,
        mut hashing_states,
        mut reuse_buffers,
        buffer_size,
        ring_size,
        receiver,
    } = state;

    let mut all_results = vec![];

    let mut no_more_files = false;

    let mut wanted = 0;

    while !no_more_files || hashing_states.len > 0 {
        //
        'get_next_fd: while hashing_states.len < ring_size {
            match receiver.try_recv() {
                Ok((entry, meta)) => {
                    let file = File::open(entry.path()).unwrap();
                    let fd = file.into_raw_fd();
                    let state = HashingState {
                        hasher: Hasher::new(),
                        buffer: reuse_buffers.get(),
                        offset: 0,
                        len: meta.len(),
                    };

                    hashing_states.insert(fd, state);

                    let read_e = opcode::Read::new(
                        io_uring::types::Fd(fd),
                        hashing_states.get_buffer(fd).as_mut_ptr() as _,
                        buffer_size,
                    )
                    .offset(0)
                    .build()
                    .user_data(fd as _);

                    unsafe {
                        ring.submission().push(&read_e).unwrap();
                    }

                    wanted += 1;
                }

                Err(crossbeam::channel::TryRecvError::Empty) => break 'get_next_fd,
                Err(crossbeam::channel::TryRecvError::Disconnected) => {
                    no_more_files = true;
                    break 'get_next_fd;
                }
            }
        }

        while !ring.completion().is_empty() {
            let cqe = ring.completion().next().unwrap();
            let fd = cqe.user_data() as i32;
            let ret = cqe.result();

            let mut state = hashing_states.remove(fd);
            state.offset += ret as u64;

            if state.offset == state.len {
                // EOF
                let _file = unsafe { File::from_raw_fd(fd) };

                let hash = state.hasher.finalize().into();

                reuse_buffers.put(state.buffer);

                all_results.push(FileInfo { hash });
            } else {
                state.hasher.update(&state.buffer[..(ret as usize)]);

                let read_e = opcode::Read::new(
                    io_uring::types::Fd(fd),
                    state.buffer.as_mut_ptr() as _,
                    buffer_size,
                )
                .offset(state.offset)
                .build()
                .user_data(fd as _);

                hashing_states.insert(fd, state);

                unsafe {
                    ring.submission().push(&read_e).unwrap();
                }

                wanted += 1;
            }
        }

        if wanted == 0 {
            ring.submit_and_wait(wanted).unwrap();
        } else {
            std::thread::yield_now();
        }
        wanted = 0;
    }

    all_results
}

struct HashingStateMap {
    keys: Vec<i32>,
    values: Vec<Option<HashingState>>,
    len: usize,
}

impl HashingStateMap {
    fn new(ring_size: usize) -> Self {
        Self {
            keys: vec![0; ring_size],
            values: std::iter::repeat_with(|| None).take(ring_size).collect(),
            len: 0,
        }
    }

    fn insert(&mut self, key: i32, value: HashingState) {
        let index = self.len;
        self.keys[index] = key;
        self.values[index] = Some(value);
        self.len += 1;
    }

    fn get_buffer(&mut self, key: i32) -> &mut [u8] {
        let index = self.keys.iter().position(|k| k == &key).unwrap();
        let value = self.values[index].as_mut().unwrap();
        &mut value.buffer
    }

    fn remove(&mut self, key: i32) -> HashingState {
        let index = self.keys.iter().position(|k| k == &key).unwrap();
        let value = self.values[index].take().unwrap();

        // swap the last element into the index we are removing
        let last_index = self.len - 1;
        self.keys.swap(index, last_index);
        self.values.swap(index, last_index);

        self.len -= 1;

        value
    }
}

struct ReuseBuffers {
    buffers: Vec<Option<Buffer>>,
    len: usize,
}

impl ReuseBuffers {
    fn new(ring_size: usize, buffer_size: usize) -> Self {
        let buffers = std::iter::repeat_with(|| Some(vec![0; buffer_size].into_boxed_slice()))
            .take(ring_size)
            .collect::<Vec<_>>();

        Self {
            buffers,
            len: ring_size,
        }
    }

    fn get(&mut self) -> Buffer {
        let index = self.len - 1;
        self.len -= 1;
        self.buffers[index].take().unwrap()
    }

    fn put(&mut self, buffer: Buffer) {
        let index = self.len;
        self.len += 1;
        self.buffers[index] = Some(buffer);
    }
}
