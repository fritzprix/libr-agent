use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Serializes `LIBRAGENT_MAX_FILE_SIZE` mutations across integration tests.
static MAX_FILE_SIZE_ENV_LOCK: OnceLock<RwLock<()>> = OnceLock::new();

fn max_file_size_env_lock() -> &'static RwLock<()> {
    MAX_FILE_SIZE_ENV_LOCK.get_or_init(|| RwLock::new(()))
}

/// Held for the duration of editFile integration tests that expect the default size limit.
pub struct EnvTestReadGuard {
    _guard: RwLockReadGuard<'static, ()>,
}

impl EnvTestReadGuard {
    pub fn acquire() -> Self {
        Self {
            _guard: max_file_size_env_lock()
                .read()
                .expect("env test read lock poisoned"),
        }
    }
}

/// RAII guard: temporarily sets an env var and restores it on drop.
pub struct EnvVarGuard {
    name: String,
    prev_value: Option<String>,
    _write: RwLockWriteGuard<'static, ()>,
}

impl EnvVarGuard {
    pub fn set_temp(name: &str, value: &str) -> Self {
        let _write = max_file_size_env_lock()
            .write()
            .expect("env test write lock poisoned");
        let prev_value = std::env::var(name).ok();
        // SAFETY: access is serialized by the write lock above.
        unsafe { std::env::set_var(name, value) };
        Self {
            name: name.to_string(),
            prev_value,
            _write,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: access is serialized by the write lock in `_write`.
        unsafe {
            if let Some(ref value) = self.prev_value {
                std::env::set_var(&self.name, value);
            } else {
                std::env::remove_var(&self.name);
            }
        }
    }
}
