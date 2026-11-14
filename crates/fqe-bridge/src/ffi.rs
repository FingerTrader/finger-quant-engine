//! Candidate C ABI: trusted host writes only its owned input allocation before call.
//! Rust never dereferences a foreign pointer. No Rust struct/layout crosses the ABI.
use std::{
    collections::BTreeMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
const MAX_BUFFERS: usize = 16;
const MAX_BYTES: usize = 2 * crate::MAX_RESPONSE;
static BUFFERS: LazyLock<Mutex<BTreeMap<usize, Box<[u8]>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static TASKS: LazyLock<Mutex<BTreeMap<u64, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);
fn insert(bytes: Box<[u8]>) -> usize {
    let Ok(mut map) = BUFFERS.lock() else {
        return 0;
    };
    if map.len() >= MAX_BUFFERS
        || map.values().map(|b| b.len()).sum::<usize>() + bytes.len() > MAX_BYTES
    {
        return 0;
    }
    let address = bytes.as_ptr() as usize;
    map.insert(address, bytes);
    address
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_bridge_abi() -> u32 {
    1
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_buffer_new(length: usize) -> usize {
    if length == 0 || length > crate::MAX_REQUEST {
        return 0;
    }
    insert(vec![0; length].into_boxed_slice())
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_buffer_len(address: usize) -> usize {
    BUFFERS
        .lock()
        .ok()
        .and_then(|m| m.get(&address).map(|b| b.len()))
        .unwrap_or(0)
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_buffer_free(address: usize) -> u32 {
    u32::from(
        BUFFERS
            .lock()
            .ok()
            .and_then(|mut m| m.remove(&address))
            .is_some(),
    )
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_buffer_count() -> usize {
    BUFFERS.lock().map(|m| m.len()).unwrap_or(usize::MAX)
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_task_new() -> u64 {
    let Ok(mut m) = TASKS.lock() else { return 0 };
    if m.len() >= 8 {
        return 0;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    if id == 0 || m.contains_key(&id) {
        return 0;
    }
    m.insert(id, Arc::new(AtomicBool::new(false)));
    id
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_task_cancel(id: u64) -> u32 {
    let Ok(m) = TASKS.lock() else { return 0 };
    if let Some(flag) = m.get(&id) {
        flag.store(true, Ordering::Release);
        1
    } else {
        0
    }
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_task_free(id: u64) -> u32 {
    u32::from(TASKS.lock().ok().and_then(|mut m| m.remove(&id)).is_some())
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_task_count() -> usize {
    TASKS.lock().map(|m| m.len()).unwrap_or(usize::MAX)
}
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn fqe_call(address: usize, length: usize, task: u64) -> usize {
    let bytes = match BUFFERS.lock() {
        Ok(m) => match m.get(&address) {
            Some(v) if v.len() == length && length <= crate::MAX_REQUEST => v.to_vec(),
            _ => return 0,
        },
        Err(_) => return 0,
    };
    let flag = match TASKS.lock() {
        Ok(m) => match m.get(&task) {
            Some(v) => Arc::clone(v),
            None => return 0,
        },
        Err(_) => return 0,
    };
    insert(crate::dispatch(&bytes, &flag).into_boxed_slice())
}
