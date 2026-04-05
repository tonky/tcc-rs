use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

use crate::dbus_client::DaemonClient;
use crate::model::PollConfig;
use crate::msg::{CpuState, DataUpdate, FanState, Msg};

/// Parsed fan curve response: (profile_name, fan_profile, curve_points).
type FanCurveResponse = (String, String, Vec<(f64, f64)>);

/// Parse get_active_fan_curve() response.
pub fn parse_fan_curve_response(json: &str) -> Option<FanCurveResponse> {
    let value = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let profile_name = value
        .get("profileName")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let fan = value.get("fan");
    let fan_profile = fan
        .and_then(|f| f.get("fanProfile"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let curve = fan
        .and_then(|f| f.get("customFanCurve"))
        .and_then(|c| c.get("tableCPU"))
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let temp = e.get("temp")?.as_f64()?;
                    let speed = e.get("speed")?.as_f64()?;
                    Some((temp, speed))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((profile_name, fan_profile, curve))
}

/// Spawns async polling tasks that send DataUpdate messages at configurable intervals.
pub struct DataPoller;

impl DataPoller {
    /// Spawns a fast-interval polling task (fan data).
    /// Returns the JoinHandle so the caller can abort on shutdown.
    pub fn spawn_fast(
        tx: mpsc::UnboundedSender<Msg>,
        client: Arc<tokio::sync::Mutex<DaemonClient>>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let result = {
                    let c = client.lock().await;
                    c.get_fan_speed_percent().await
                };

                match result {
                    Ok(speed) => {
                        let fan = FanState {
                            speeds_percent: vec![speed],
                        };
                        let _ = tx.send(Msg::Data(DataUpdate::FanData(fan)));
                    }
                    Err(_) => {
                        let _ = tx.send(Msg::Data(DataUpdate::ConnectionLost));
                        // Try to reconnect
                        let mut c = client.lock().await;
                        if c.connect().await.is_ok() {
                            let _ = tx.send(Msg::Data(DataUpdate::ConnectionRestored));
                        }
                    }
                }

                sleep(interval).await;
            }
        })
    }

    /// Spawns a medium-interval polling task (CPU info, power state, fan curve).
    pub fn spawn_medium(
        tx: mpsc::UnboundedSender<Msg>,
        client: Arc<tokio::sync::Mutex<DaemonClient>>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                // CPU info
                {
                    let c = client.lock().await;
                    if let Ok(json) = c.get_cpu_info().await
                        && let Ok(info) = serde_json::from_str::<serde_json::Value>(&json)
                    {
                        let cpu = CpuState {
                            temperature: info
                                .get("temperature")
                                .and_then(|v| v.as_f64()),
                            avg_frequency_mhz: info
                                .get("avgFrequencyMhz")
                                .and_then(|v| v.as_f64()),
                            core_count: info
                                .get("coreCount")
                                .and_then(|v| v.as_u64())
                                .map(|v| v as usize),
                        };
                        let _ = tx.send(Msg::Data(DataUpdate::CpuMetrics(cpu)));
                    }
                }
                // Power state
                {
                    let c = client.lock().await;
                    if let Ok(state) = c.get_power_state().await {
                        let _ = tx.send(Msg::Data(DataUpdate::PowerState {
                            on_ac: state == "ac",
                        }));
                    }
                }
                // Active fan curve
                {
                    let c = client.lock().await;
                    if let Ok(json) = c.get_active_fan_curve().await
                        && let Some((profile_name, fan_profile, curve)) =
                            parse_fan_curve_response(&json)
                    {
                        let _ =
                            tx.send(Msg::Data(DataUpdate::FanCurveData {
                                profile_name,
                                fan_profile,
                                curve_cpu: curve,
                            }));
                    }
                }

                sleep(interval).await;
            }
        })
    }

    /// Spawns all polling tasks with the given config.
    /// Returns handles for cleanup.
    pub fn spawn_all(
        tx: mpsc::UnboundedSender<Msg>,
        client: Arc<tokio::sync::Mutex<DaemonClient>>,
        config: &PollConfig,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        // Fast poll: fan data, Medium poll: CPU + power + fan curve
        vec![
            Self::spawn_fast(tx.clone(), client.clone(), config.fast),
            Self::spawn_medium(tx.clone(), client.clone(), config.medium),
        ]
    }
}
