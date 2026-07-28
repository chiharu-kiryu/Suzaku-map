#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ScopedEnv {
    originals: Vec<(String, Option<String>)>,
}

#[cfg(test)]
impl ScopedEnv {
    pub(crate) fn new() -> Self {
        Self {
            originals: Vec::new(),
        }
    }

    pub(crate) fn set_var(&mut self, key: &str, value: &str) -> &mut Self {
        let previous = std::env::var(key).ok();
        self.originals.push((key.to_string(), previous));
        unsafe {
            std::env::set_var(key, value);
        }
        self
    }

    pub(crate) fn remove_var(&mut self, key: &str) -> &mut Self {
        let previous = std::env::var(key).ok();
        self.originals.push((key.to_string(), previous));
        unsafe {
            std::env::remove_var(key);
        }
        self
    }
}

#[cfg(test)]
impl Drop for ScopedEnv {
    fn drop(&mut self) {
        for (key, previous) in self.originals.drain(..).rev() {
            match previous {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}

#[cfg(test)]
fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
fn with_env_mutex() -> MutexGuard<'static, ()> {
    match env_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
pub(crate) fn with_test_env<R>(f: impl FnOnce(&mut ScopedEnv) -> R) -> R {
    let _lock = with_env_mutex();
    let mut env = ScopedEnv::new();
    f(&mut env)
}
