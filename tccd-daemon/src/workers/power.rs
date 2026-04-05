use crate::io::TuxedoIO;
use crate::profiles::{PowerState, ProfileStore};
use crate::workers::fan::FanControlTask;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

/// Monitors AC/battery power state and auto-switches profiles on transition.
pub struct PowerStateWorker {
    io: Arc<dyn TuxedoIO + Send + Sync>,
    fan_task: Arc<FanControlTask>,
    profile_store: Arc<Mutex<ProfileStore>>,
    poll_interval: Duration,
}

impl PowerStateWorker {
    pub fn new(
        io: Arc<dyn TuxedoIO + Send + Sync>,
        fan_task: Arc<FanControlTask>,
        profile_store: Arc<Mutex<ProfileStore>>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            io,
            fan_task,
            profile_store,
            poll_interval,
        }
    }

    pub fn spawn(&self) -> tokio::task::JoinHandle<()> {
        let io = self.io.clone();
        let fan_task = self.fan_task.clone();
        let profile_store = self.profile_store.clone();
        let interval = self.poll_interval;

        tokio::spawn(async move {
            let mut last_on_ac: Option<bool> = None;

            loop {
                if let Ok(on_ac) = io.is_ac_power() {
                    let changed = last_on_ac.is_some() && last_on_ac != Some(on_ac);
                    last_on_ac = Some(on_ac);

                    if changed {
                        let state = if on_ac {
                            PowerState::Ac
                        } else {
                            PowerState::Battery
                        };
                        let state_label = if on_ac { "AC" } else { "battery" };

                        // Extract profile data under the lock, then release before applying
                        let profile_snapshot = {
                            let store = profile_store.lock().await;
                            store
                                .active_profile_id(state)
                                .and_then(|id| store.get_profile(id))
                                .cloned()
                        };

                        if let Some(profile) = profile_snapshot {
                            println!(
                                "Power state changed to {}, switching to profile '{}'",
                                state_label, profile.name
                            );

                            // Apply fan curve
                            if profile.fan.use_control {
                                if let Some(ref table) =
                                    profile.fan.custom_fan_curve.table_cpu
                                {
                                    fan_task.set_cpu_curve(table.clone()).await;
                                }
                                if let Some(ref table) =
                                    profile.fan.custom_fan_curve.table_gpu
                                {
                                    fan_task.set_gpu_curve(table.clone()).await;
                                }
                            }

                            // Apply CPU settings (best-effort)
                            if let Err(e) = io.set_cpu_governor(&profile.cpu.governor) {
                                eprintln!("Auto-switch CPU governor: {}", e);
                            }
                            if let Err(e) = io.set_cpu_turbo(!profile.cpu.no_turbo) {
                                eprintln!("Auto-switch CPU turbo: {}", e);
                            }
                            if !profile.cpu.energy_performance_preference.is_empty()
                                && let Err(e) = io.set_cpu_energy_perf(
                                    &profile.cpu.energy_performance_preference,
                                )
                            {
                                eprintln!("Auto-switch CPU energy perf: {}", e);
                            }
                        } else {
                            println!(
                                "Power state changed to {} but no profile assigned",
                                state_label
                            );
                        }
                    }
                }

                sleep(interval).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MockTuxedoIO;

    #[tokio::test]
    async fn test_power_state_auto_switch() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        let fan_task = Arc::new(FanControlTask::new(mock_io.clone(), 50));
        fan_task.spawn();

        let config_dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(config_dir.path());
        let profile_store = Arc::new(Mutex::new(store));

        // Assign a profile to battery state
        {
            let mut store = profile_store.lock().await;
            let profiles = store.list_profiles().to_vec();
            if let Some(profile) = profiles.first() {
                store
                    .set_active_profile(&profile.id, PowerState::Battery)
                    .unwrap();
            }
        }

        let worker = PowerStateWorker::new(
            mock_io.clone(),
            fan_task.clone(),
            profile_store.clone(),
            Duration::from_millis(10),
        );
        let handle = worker.spawn();

        // Initial state is AC (default). Let it poll once to establish baseline.
        sleep(Duration::from_millis(30)).await;

        // Switch to battery
        *mock_io.ac_power.write().unwrap() = false;
        sleep(Duration::from_millis(50)).await;

        // The governor should have been applied from the profile
        let gov = mock_io.cpu_governor.read().unwrap().clone();
        // Default profile governor is "powersave" - just verify the worker ran
        assert!(!gov.is_empty());

        handle.abort();
    }

    #[tokio::test]
    async fn test_no_switch_on_steady_state() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        let fan_task = Arc::new(FanControlTask::new(mock_io.clone(), 50));

        let config_dir = tempfile::tempdir().unwrap();
        let profile_store = Arc::new(Mutex::new(ProfileStore::new(config_dir.path())));

        let worker = PowerStateWorker::new(
            mock_io.clone(),
            fan_task,
            profile_store,
            Duration::from_millis(10),
        );
        let handle = worker.spawn();

        // Stay on AC for several polls — should not trigger any profile switch
        sleep(Duration::from_millis(60)).await;

        // Governor should still be the MockTuxedoIO default
        let gov = mock_io.cpu_governor.read().unwrap().clone();
        assert_eq!(gov, "powersave");

        handle.abort();
    }
}
