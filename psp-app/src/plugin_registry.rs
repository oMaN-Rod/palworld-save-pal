//! Bundled plugin sources and the in-flight cancellation handles for running
//! plugin commands.

/// One plugin shipped inside the binary. `manifest` and `sources` are
/// `include_str!`'d at compile time by whoever populates `BUNDLED`.
pub struct BundledPlugin {
    pub id: &'static str,
    pub manifest: &'static str,
    pub sources: &'static [(&'static str, &'static str)],
}

pub const BUNDLED: &[BundledPlugin] = &[
    BundledPlugin {
        id: "pst.cleanup",
        manifest: include_str!("bundled/pst.cleanup/manifest.json"),
        sources: &[("main.lua", include_str!("bundled/pst.cleanup/main.lua"))],
    },
    BundledPlugin {
        id: "pst.reset",
        manifest: include_str!("bundled/pst.reset/manifest.json"),
        sources: &[("main.lua", include_str!("bundled/pst.reset/main.lua"))],
    },
    BundledPlugin {
        id: "pst.tools",
        manifest: include_str!("bundled/pst.tools/manifest.json"),
        sources: &[("main.lua", include_str!("bundled/pst.tools/main.lua"))],
    },
];

/// Holds a `Cancel` handle per in-flight run, keyed by run id, so a
/// connection other than the one that started a run can still cancel it.
#[derive(Default)]
pub struct PluginRegistry {
    runs: std::sync::Mutex<std::collections::HashMap<uuid::Uuid, psp_plugin::sandbox::Cancel>>,
}

impl PluginRegistry {
    pub fn bundled(&self) -> &'static [BundledPlugin] {
        BUNDLED
    }

    /// Registers a fresh run's cancellation handle. Overwrites silently if
    /// `run_id` were ever reused, which it never is (`Uuid::new_v4`).
    pub fn register_run(&self, run_id: uuid::Uuid, cancel: psp_plugin::sandbox::Cancel) {
        let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        runs.insert(run_id, cancel);
    }

    /// Signals cancellation for `run_id` if it is still in flight. Returns
    /// `false` for an unknown id — a cancel arriving after the run finished
    /// is normal, not an error.
    pub fn cancel_run(&self, run_id: uuid::Uuid) -> bool {
        let runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        match runs.get(&run_id) {
            Some(cancel) => {
                cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Removes a run's handle once it has finished, whatever the outcome.
    pub fn finish_run(&self, run_id: uuid::Uuid) {
        let mut runs = self.runs.lock().unwrap_or_else(|e| e.into_inner());
        runs.remove(&run_id);
    }
}
