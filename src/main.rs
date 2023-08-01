use types::Outputs;

use crate::types::Inputs;

mod types;

mod rayon_thread_pool;

#[cfg(target_os = "linux")]
mod io_uring;
#[cfg(target_os = "linux")]
mod io_uring_batched;

mod limits;

struct Strategy {
    name: &'static str,
    description: &'static str,
    run: fn(Inputs) -> Outputs,
}

const STRATEGIES: &[Strategy] = &[
    Strategy {
        name: "rayon_thread_pool",
        description: "implementation with a rayon thread pool.",
        run: rayon_thread_pool::main,
    },
    #[cfg(target_os = "linux")]
    Strategy {
        name: "io_uring",
        description: "implementation with a thread pool and io_uring. Each thread processes one IO request at a time.",
        run: io_uring::main,
    },
    #[cfg(target_os = "linux")]
    Strategy {
        name: "io_uring_batched",
        description: "implementation with a thread pool and io_uring. Each thread processes multiple IO requests at a time.",
        run: io_uring_batched::main,
    },
];

fn main() {
    let strategy_str = std::env::args().nth(1).expect("Missing strategy");
    let directory = std::env::args().nth(2).expect("Missing directory");

    let num_threads = std::env::var("NUM_THREADS")
        .map(|s| s.parse::<usize>().expect("Invalid NUM_THREADS"))
        .unwrap_or_else(|_| num_cpus::get());

    let buffer_size = std::env::var("BUFFER_SIZE")
        .map(|s| s.parse::<usize>().expect("Invalid BUFFER_SIZE"))
        .unwrap_or(4096);

    let io_uring_ring_size = std::env::var("IO_URING_RING_SIZE")
        .map(|s| s.parse::<usize>().expect("Invalid IO_URING_RING_SIZE"))
        .unwrap_or(32);

    let strategy = STRATEGIES.iter().find(|s| s.name == strategy_str.as_str());

    let executor = if let Some(strategy) = strategy {
        strategy.run
    } else {
        eprintln!("Unknown strategy: {}", strategy_str);
        eprintln!("Available strategies:");
        for strategy in STRATEGIES {
            eprintln!("  {} - {}", strategy.name, strategy.description);
        }
        std::process::exit(1);
    };

    let inputs = Inputs {
        directory: std::path::PathBuf::from(directory),
        threads: num_threads,
        max_fds: limits::max_fds() as usize,
        buffer_size,
        io_uring_ring_size,
    };

    eprintln!("About to start with parameters");
    eprintln!("  directory: {}", inputs.directory.display());
    eprintln!("  threads: {}", inputs.threads);
    eprintln!("  max_fds: {}", inputs.max_fds);
    eprintln!("  buffer_size: {}", inputs.buffer_size);
    eprintln!("  io_uring_ring_size: {}", inputs.io_uring_ring_size);

    let Outputs {
        start,
        files: results,
    } = executor(inputs);

    let elapsed = start.elapsed();

    eprintln!("{} files in {}ms", results.len(), elapsed.as_millis());

    // Force utilization of the results to avoid the compiler optimizing away.
    for result in results {
        println!("{:?}", result.hash);
    }
}
