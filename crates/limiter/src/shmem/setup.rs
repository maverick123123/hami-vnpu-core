use std::ffi::CString;
use std::mem;
use std::ptr;
use std::path::Path;

use libc::{open, close, ftruncate, mmap, O_RDWR, O_CREAT, PROT_READ, PROT_WRITE, MAP_SHARED, MAP_FAILED, off_t, mode_t};
use libc::{mkdir, O_RDONLY};

use crate::shmem::GlobalRegistry;

/// Create a file-backed shared memory region.
/// `path` is a full filesystem path; parent directories are created if missing.
pub fn create_shmem<T>(path: &str) -> &'static T {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        if !parent.exists() {
            let c_parent = CString::new(parent.to_str().unwrap()).unwrap();
            unsafe { mkdir(c_parent.as_ptr(), 0o777 as mode_t) };
        }
    }

    unsafe {
        let c_path = CString::new(path).unwrap();
        let fd = open(c_path.as_ptr(), O_CREAT | O_RDWR, 0o666 as mode_t);
        if fd < 0 { panic!("Manager failed to create shmem file {}: {}", path, std::io::Error::last_os_error()); }

        let size = mem::size_of::<T>();
        if ftruncate(fd, size as off_t) < 0 {
            panic!("Failed to ftruncate {}: {}", path, std::io::Error::last_os_error());
        }

        let ptr = mmap(ptr::null_mut(), size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        if ptr == MAP_FAILED { panic!("Failed to mmap {}: {}", path, std::io::Error::last_os_error()); }

        close(fd);
        &*(ptr as *const T)
    }
}

/// Open an existing file-backed shared memory (panics on failure).
pub fn open_shmem<T>(path: &str) -> &'static T {
    try_open_shmem(path).expect("Worker failed to open NPU Manager shmem! Is the Daemon running?")
}

/// Non-panicking version: returns None if the shmem file is not available.
pub fn try_open_shmem<T>(path: &str) -> Option<&'static T> {
    unsafe {
        let c_path = CString::new(path).unwrap();
        let fd = open(c_path.as_ptr(), O_RDWR);
        if fd < 0 { return None; }

        let ptr = mmap(ptr::null_mut(), mem::size_of::<T>(), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        close(fd);
        if ptr == MAP_FAILED { return None; }
        Some(&*(ptr as *const T))
    }
}

pub fn open_global_registry(path: &str) -> &'static GlobalRegistry {
    let c_path = CString::new(path).unwrap();
    println!("open global registry path is {:?}", path);
    let mut fd = unsafe { open(c_path.as_ptr(), O_RDWR) };
    let mut needs_init = false;

    if fd < 0 {
        // File not exist: First Pod
        println!("[Global] Global Registry not exist, now creating...");
        fd = unsafe { open(c_path.as_ptr(), O_RDWR | O_CREAT, 0o666) };
        if fd < 0 { panic!("cannot open: {}", path); }
        needs_init = true;
    }

    let size = std::mem::size_of::<GlobalRegistry>();

    if needs_init {
        unsafe { ftruncate(fd, size as i64) };
    }

    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            0,
        )
    };

    if ptr == MAP_FAILED { panic!("mmap failed"); }

    let reg = unsafe { &*(ptr as *const GlobalRegistry) };

    if needs_init {
        // TODO
        // 这里手动调用初始化逻辑，比如把锁所有权设为某个无效值
        // (reg as *mut GlobalRegistry).initialize_fields();
    }
    println!("connect to global registry");
    reg
}

