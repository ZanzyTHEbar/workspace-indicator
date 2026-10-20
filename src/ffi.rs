#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void};

pub type size_t = usize;
pub type ssize_t = isize;
pub type off_t = i64;
pub type pid_t = c_int;
pub type time_t = i64;
pub type clockid_t = c_int;

pub const LOCK_EX: c_int = 2;
pub const LOCK_NB: c_int = 4;

pub const SIGINT: c_int = 2;
pub const SIGUSR1: c_int = 10;
pub const SIGUSR2: c_int = 12;
pub const SIGTERM: c_int = 15;
pub const SIG_BLOCK: c_int = 0;

pub const CLOCK_MONOTONIC: clockid_t = 1;
pub const TFD_NONBLOCK: c_int = 0o4000;
pub const TFD_CLOEXEC: c_int = 0o2000000;

pub const SFD_NONBLOCK: c_int = 0o4000;
pub const SFD_CLOEXEC: c_int = 0o2000000;

pub const IN_NONBLOCK: c_int = 0o4000;
pub const IN_CLOEXEC: c_int = 0o2000000;
pub const IN_MODIFY: u32 = 0x0000_0002;
pub const IN_ATTRIB: u32 = 0x0000_0004;
pub const IN_CLOSE_WRITE: u32 = 0x0000_0008;
pub const IN_MOVED_TO: u32 = 0x0000_0080;
pub const IN_CREATE: u32 = 0x0000_0100;
pub const IN_DELETE: u32 = 0x0000_0200;

pub const EPOLL_CTL_ADD: c_int = 1;
pub const EPOLL_CTL_DEL: c_int = 2;
pub const EPOLL_CTL_MOD: c_int = 3;
pub const EPOLLIN: u32 = 0x001;
pub const EPOLLERR: u32 = 0x008;
pub const EPOLLHUP: u32 = 0x010;
pub const EPOLL_CLOEXEC: c_int = 0o2000000;

pub const MFD_CLOEXEC: c_uint = 0x0001;
pub const PROT_READ: c_int = 0x1;
pub const PROT_WRITE: c_int = 0x2;
pub const MAP_SHARED: c_int = 0x01;
pub const MAP_FAILED: *mut c_void = !0usize as *mut c_void;

pub const SOL_SOCKET: c_int = 1;
pub const SCM_RIGHTS: c_int = 0x01;
pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_NOSIGNAL: c_int = 0x4000;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct sigset_t {
    pub __val: [c_ulong; 16],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct itimerspec {
    pub it_interval: timespec,
    pub it_value: timespec,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct epoll_event {
    pub events: u32,
    pub u64: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct signalfd_siginfo {
    pub ssi_signo: u32,
    pub ssi_errno: i32,
    pub ssi_code: i32,
    pub ssi_pid: u32,
    pub ssi_uid: u32,
    pub ssi_fd: i32,
    pub ssi_tid: u32,
    pub ssi_band: u32,
    pub ssi_overrun: u32,
    pub ssi_trapno: u32,
    pub ssi_status: i32,
    pub ssi_int: i32,
    pub ssi_ptr: u64,
    pub ssi_utime: u64,
    pub ssi_stime: u64,
    pub ssi_addr: u64,
    pub ssi_addr_lsb: u16,
    pub __pad2: u16,
    pub ssi_syscall: i32,
    pub ssi_call_addr: u64,
    pub ssi_arch: u32,
    pub __pad: [u8; 28],
}

#[repr(C)]
pub struct iovec {
    pub iov_base: *mut c_void,
    pub iov_len: size_t,
}

#[repr(C)]
pub struct msghdr {
    pub msg_name: *mut c_void,
    pub msg_namelen: u32,
    pub msg_iov: *mut iovec,
    pub msg_iovlen: size_t,
    pub msg_control: *mut c_void,
    pub msg_controllen: size_t,
    pub msg_flags: c_int,
}

#[repr(C)]
pub struct cmsghdr {
    pub cmsg_len: size_t,
    pub cmsg_level: c_int,
    pub cmsg_type: c_int,
}

extern "C" {
    pub fn flock(fd: c_int, operation: c_int) -> c_int;
    pub fn kill(pid: pid_t, sig: c_int) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t;
    pub fn ftruncate(fd: c_int, length: off_t) -> c_int;

    pub fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
    pub fn mmap(
        addr: *mut c_void,
        length: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, length: size_t) -> c_int;

    pub fn sendmsg(sockfd: c_int, msg: *const msghdr, flags: c_int) -> ssize_t;

    pub fn epoll_create1(flags: c_int) -> c_int;
    pub fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut epoll_event) -> c_int;
    pub fn epoll_wait(
        epfd: c_int,
        events: *mut epoll_event,
        maxevents: c_int,
        timeout: c_int,
    ) -> c_int;

    pub fn timerfd_create(clockid: clockid_t, flags: c_int) -> c_int;
    pub fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec,
    ) -> c_int;

    pub fn sigemptyset(set: *mut sigset_t) -> c_int;
    pub fn sigaddset(set: *mut sigset_t, signum: c_int) -> c_int;
    pub fn sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
    pub fn signalfd(fd: c_int, mask: *const sigset_t, flags: c_int) -> c_int;

    pub fn inotify_init1(flags: c_int) -> c_int;
    pub fn inotify_add_watch(fd: c_int, pathname: *const c_char, mask: u32) -> c_int;
}
