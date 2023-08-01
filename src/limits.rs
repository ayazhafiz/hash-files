use nix::libc::{getrlimit, rlimit, RLIMIT_NOFILE};

pub fn max_fds() -> u64 {
    let mut rlimit = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    unsafe { getrlimit(RLIMIT_NOFILE as _, &mut rlimit) };
    rlimit.rlim_cur
}
