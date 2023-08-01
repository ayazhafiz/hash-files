use std::{
    fs::{self},
    path::PathBuf,
};

use rayon::Scope;
use semaphore::{Semaphore, TryAccessError};

use crate::types::{FileInfo, Inputs, Outputs};

// Derived from the `fclones` project, Copyright (c) Piotr Kołaczkowski.
// Licensed under the MIT license.
//
// The relevant source code can be found at
//   https://github.com/pkolaczk/fclones/blob/3cbe95a495480760c3ed58238587598dd34804bf/fclones/src/walk.rs#L153
//
// The license can be found at
//
//   https://github.com/pkolaczk/fclones/blob/3cbe95a495480760c3ed58238587598dd34804bf/LICENSE
pub fn main(input: Inputs) -> Outputs {
    let Inputs {
        directory: search_dir,
        threads,
        max_fds,
        buffer_size: _,
        io_uring_ring_size: _,
    } = input;

    let semaphore = Semaphore::new(max_fds * 80 / 100, ());

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();

    let walker = Walker {
        all_files: Default::default(),
        semaphore,
    };

    let start = std::time::Instant::now();

    rayon::scope(|scope| walker.visit_entry(search_dir, scope));

    let files = walker.all_files.into_iter().collect();
    Outputs { start, files }
}

struct Walker {
    all_files: boxcar::Vec<FileInfo>,
    semaphore: Semaphore<()>,
}

impl Walker {
    fn visit_entry<'a>(&'a self, path: PathBuf, scope: &Scope<'a>) {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let ft = metadata.file_type();

        if ft.is_file() {
            self.visit_file(path);
        } else if ft.is_dir() {
            self.visit_dir(path, scope);
        } else {
            //
        }
    }

    fn visit_file(&self, path: PathBuf) {
        let _permit = loop {
            match self.semaphore.try_access() {
                Ok(permit) => break permit,
                Err(TryAccessError::NoCapacity) => {
                    //
                    rayon::yield_now();
                    continue;
                }
                Err(TryAccessError::Shutdown) => panic!("Semaphore closed"),
            }
        };

        let mut file =
            std::fs::File::open(path).unwrap_or_else(|_| panic!("Failed to open file for reading"));

        let mut hasher = blake3::Hasher::new();
        std::io::copy(&mut file, &mut hasher).unwrap();
        let hash = hasher.finalize().into();

        self.all_files.push(FileInfo { hash });
    }

    fn visit_dir<'a>(&'a self, path: PathBuf, scope: &Scope<'a>) {
        match fs::read_dir(path) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    scope.spawn(move |s| self.visit_entry(entry.path(), s))
                }
            }
            Err(_e) => {
                // pass
            }
        }
    }
}
