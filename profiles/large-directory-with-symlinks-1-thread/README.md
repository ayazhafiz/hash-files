These profiles were collected in directory created by the

  make-big-target.sh

script in the root of this directory.

Total number of files: 328156

Parameters:

threads: 1
buffer_size: 8192
io_uring_ring_size: 32

Traces:

```
root@32874ee9b31428:/big-target# perf record -F 99 -a -g -o ../rayon_thread_pool.data -- /opt/mint/hash-files rayon_thread_pool . >/dev/null
About to start with parameters
  directory: .
  threads: 1
  max_fds: 10240
  buffer_size: 8192
  io_uring_ring_size: 32
328155 files in 38295ms

root@32874ee9b31428:/big-target# perf record -F 99 -a -g -o ../io_uring.data -- /opt/mint/hash-files io_uring . >/dev/null
About to start with parameters
  directory: .
  threads: 1
  max_fds: 10240
  buffer_size: 8192
  io_uring_ring_size: 32
328155 files in 39651ms

root@32874ee9b31428:/big-target# perf record -F 99 -a -g -o ../io_uring_batched.data -- /opt/mint/hash-files io_uring_batched . >/dev/null
About to start with parameters
  directory: .
  threads: 1
  max_fds: 10240
  buffer_size: 8192
  io_uring_ring_size: 32
328155 files in 45088ms
```
