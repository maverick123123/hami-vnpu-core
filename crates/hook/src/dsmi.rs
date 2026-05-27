use crate::npu_limiter;

// ============================================================
// Structs (shared by DSMI and DCMI hooks)
// ============================================================

// dsmi_get_memory_info_v2 struct (KB units, verified via C test)
#[repr(C)]
pub struct DsmiMemoryInfoV2 {
    pub memory_size: u64,
    pub memory_available: u64,
}

// dsmi_get_hbm_info / dcmi_get_device_hbm_info struct (KB)
#[repr(C)]
pub struct HbmInfo {
    pub memory_size: u64,
    pub freq: u32,
    pub memory_usage: u64,
    pub temp: i32,
    pub bandwith_util_rate: u32,
}

// dcmi_get_memory_info struct (MB units)
#[repr(C)]
pub struct DcmiMemoryInfoStru {
    pub memory_size: u64,
    pub freq: u32,
    pub utiliza: u32,
}

// dcmi_get_device_memory_info_v3 struct (MB, default npu-smi view)
#[repr(C)]
pub struct DcmiMemoryInfoV3 {
    pub memory_size: u64,
    pub memory_available: u64,
    pub freq: u32,
    pub hugepagesize: u64,
    pub hugepages_total: u64,
    pub hugepages_free: u64,
    pub utiliza: u32,
    pub reserve: [u8; 60],
}

// ============================================================
// Passthrough macro
// ============================================================

macro_rules! hook_passthrough {
    ($name:expr, ($($sig:tt)*), $($arg:expr),*) => {
        {
            static REAL: ::once_cell::sync::Lazy<extern "C" fn($($sig)*) -> i32> =
                ::once_cell::sync::Lazy::new(|| unsafe {
                    let ptr = libc::dlsym(
                        libc::RTLD_NEXT,
                        concat!($name, "\0").as_ptr() as *const libc::c_char,
                    );
                    if ptr.is_null() {
                        panic!("cannot find original function: {}", $name);
                    }
                    std::mem::transmute(ptr)
                });
            (*REAL)($($arg),*)
        }
    };
}

// ============================================================
// Memory hooks
// ============================================================

// DSMI — npu-smi info -t memory subcommand
#[unsafe(no_mangle)]
pub extern "C" fn dsmi_get_memory_info_v2(device_id: i32, info: *mut DsmiMemoryInfoV2) -> i32 {
    let ret = hook_passthrough!("dsmi_get_memory_info_v2", (i32, *mut DsmiMemoryInfoV2), device_id, info);
    if npu_limiter().is_hbm_limited() && ret == 0 {
        let used = npu_limiter().recalculate_usage_for_device(device_id as usize);
        let quota = npu_limiter().get_hbm_quota();
        let quota_kb = quota / 1024;
        let used_kb = used / 1024;
        let available_kb = if quota_kb > used_kb { quota_kb - used_kb } else { 0 };
        unsafe { (*info).memory_size = quota_kb; (*info).memory_available = available_kb; }
    }
    ret
}

// DCMI — used by default npu-smi view
#[unsafe(no_mangle)]
pub extern "C" fn dcmi_get_device_memory_info_v3(card_id: i32, device_id: i32, info: *mut DcmiMemoryInfoV3) -> i32 {
    let ret = hook_passthrough!("dcmi_get_device_memory_info_v3", (i32, i32, *mut DcmiMemoryInfoV3), card_id, device_id, info);
    if npu_limiter().is_hbm_limited() && ret == 0 {
        let used = npu_limiter().recalculate_usage_for_device(device_id as usize);
        let quota = npu_limiter().get_hbm_quota();
        let quota_mb = quota / (1024 * 1024);
        let used_mb = used / (1024 * 1024);
        let available_mb = if quota_mb > used_mb { quota_mb - used_mb } else { 0 };
        unsafe { (*info).memory_size = quota_mb; (*info).memory_available = available_mb; }
    }
    ret
}

// DCMI — alternative memory query path
#[unsafe(no_mangle)]
pub extern "C" fn dcmi_get_memory_info(card_id: i32, device_id: i32, info: *mut DcmiMemoryInfoStru) -> i32 {
    let ret = hook_passthrough!("dcmi_get_memory_info", (i32, i32, *mut DcmiMemoryInfoStru), card_id, device_id, info);
    if npu_limiter().is_hbm_limited() && ret == 0 {
        let used = npu_limiter().recalculate_usage_for_device(device_id as usize);
        let quota = npu_limiter().get_hbm_quota();
        let quota_mb = quota / (1024 * 1024);
        let used_mb = used / (1024 * 1024);
        unsafe {
            (*info).memory_size = quota_mb;
            (*info).utiliza = if quota_mb > 0 { (used_mb * 100 / quota_mb) as u32 } else { 0 };
        }
    }
    ret
}

// ============================================================
// HBM hooks
// ============================================================

#[unsafe(no_mangle)]
pub extern "C" fn dsmi_get_hbm_info(device_id: i32, info: *mut HbmInfo) -> i32 {
    let ret = hook_passthrough!("dsmi_get_hbm_info", (i32, *mut HbmInfo), device_id, info);
    if npu_limiter().is_hbm_limited() && ret == 0 {
        let used = npu_limiter().recalculate_usage_for_device(device_id as usize);
        let quota = npu_limiter().get_hbm_quota();
        let quota_kb = quota / 1024;
        let used_kb = used / 1024;
        unsafe { (*info).memory_size = quota_kb; (*info).memory_usage = used_kb; }
    }
    ret
}

#[unsafe(no_mangle)]
pub extern "C" fn dcmi_get_device_hbm_info(card_id: i32, device_id: i32, info: *mut HbmInfo) -> i32 {
    let ret = hook_passthrough!("dcmi_get_device_hbm_info", (i32, i32, *mut HbmInfo), card_id, device_id, info);
    if npu_limiter().is_hbm_limited() && ret == 0 {
        let used = npu_limiter().recalculate_usage_for_device(device_id as usize);
        let quota = npu_limiter().get_hbm_quota();
        let quota_kb = quota / 1024;
        let used_kb = used / 1024;
        unsafe { (*info).memory_size = quota_kb; (*info).memory_usage = used_kb; }
    }
    ret
}

// ============================================================
// AICore utilization hooks
// ============================================================

#[unsafe(no_mangle)]
pub extern "C" fn dsmi_get_device_utilization_rate(device_id: i32, device_type: i32, rate: *mut u32) -> i32 {
    let ret = hook_passthrough!("dsmi_get_device_utilization_rate", (i32, i32, *mut u32), device_id, device_type, rate);
    if ret == 0 && !rate.is_null() {
        unsafe { *rate = npu_limiter().get_compute_share(*rate); }
    }
    ret
}

#[unsafe(no_mangle)]
pub extern "C" fn dcmi_get_device_utilization_rate(card_id: i32, device_id: i32, input_type: i32, rate: *mut u32) -> i32 {
    let ret = hook_passthrough!("dcmi_get_device_utilization_rate", (i32, i32, i32, *mut u32), card_id, device_id, input_type, rate);
    if ret == 0 && !rate.is_null() {
        unsafe { *rate = npu_limiter().get_compute_share(*rate); }
    }
    ret
}
