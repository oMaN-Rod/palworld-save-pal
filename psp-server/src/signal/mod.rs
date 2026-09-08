pub mod broker_client;
pub mod crypto;
pub mod framing;
pub mod gamedata_source;
pub mod manager;
pub mod peer;
pub mod remote_ctl;
pub mod rest_source;

#[cfg(test)]
pub(crate) mod test_support {
    static SIGNAL_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    pub(crate) struct SignalEnvGuard {
        _lock: tokio::sync::MutexGuard<'static, ()>,
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl SignalEnvGuard {
        pub(crate) async fn acquire(vars: &[(&'static str, Option<&str>)]) -> Self {
            let lock = SIGNAL_ENV_LOCK.lock().await;
            let mut previous = Vec::new();
            for (name, value) in vars {
                previous.push((*name, std::env::var_os(name)));
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for SignalEnvGuard {
        fn drop(&mut self) {
            for (name, prior) in &self.previous {
                match prior {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}
