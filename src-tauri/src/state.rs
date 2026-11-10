use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard};

use crate::store::Store;

pub struct AppState {
    pub store: Mutex<Store>,
    pub allow_close: Mutex<HashSet<String>>,
    pub opening: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn load() -> Self {
        Self {
            store: Mutex::new(Store::load()),
            allow_close: Mutex::new(HashSet::new()),
            opening: Mutex::new(HashSet::new()),
        }
    }
}

pub fn lock_store(state: &AppState) -> MutexGuard<'_, Store> {
    state
        .store
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub fn lock_set(set: &Mutex<HashSet<String>>) -> MutexGuard<'_, HashSet<String>> {
    set.lock().unwrap_or_else(|poison| poison.into_inner())
}
