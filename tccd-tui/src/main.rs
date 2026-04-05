mod command;
mod dbus_client;
mod model;
mod msg;
mod poller;
mod update;
mod views;
mod widgets;

use std::sync::Arc;

use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::command::Command;
use crate::dbus_client::DaemonClient;
use crate::model::Model;
use crate::msg::Msg;
use crate::poller::DataPoller;

/// Parse --bus=session flag for development (default: system bus).
fn use_session_bus() -> bool {
    std::env::args().any(|a| a == "--bus=session" || a == "--session")
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let result = run(terminal).await;
    ratatui::restore();
    result
}

async fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut model = Model::default();

    // D-Bus client
    let session = use_session_bus();
    let client = Arc::new(tokio::sync::Mutex::new(DaemonClient::new(session)));

    // Initial connection attempt
    {
        let mut c = client.lock().await;
        if c.connect().await.is_ok() {
            model.connection_status = crate::model::ConnectionStatus::Connected;
        }
    }

    // Message channel
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    // Spawn data polling tasks
    let poller_handles = DataPoller::spawn_all(tx.clone(), client.clone(), &model.poll_config);

    // Clone tx for the command dispatcher
    let cmd_tx = tx.clone();
    let cmd_client = client.clone();

    // Async crossterm event stream
    let mut event_stream = EventStream::new();

    // Main TEA loop
    loop {
        // 1. Render if dirty
        if model.dirty {
            terminal.draw(|frame| views::view(&model, frame))?;
            model.dirty = false;
        }

        // 2. Collect messages — async select over terminal events and data channel
        let msg = tokio::select! {
            // Terminal events (keyboard, resize)
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => Some(Msg::Key(key)),
                    Some(Ok(Event::Resize(w, h))) => Some(Msg::Resize(w, h)),
                    _ => None,
                }
            }
            // Data updates from poller
            Some(data_msg) = rx.recv() => {
                Some(data_msg)
            }
        };

        // 3. Process message through TEA update + dispatch commands
        if let Some(msg) = msg {
            dispatch_commands(
                update::update(&mut model, msg),
                &poller_handles,
                &cmd_client,
                &cmd_tx,
            )
            .await?;
        }

        // Drain any queued messages, dispatching their commands too
        while let Ok(queued_msg) = rx.try_recv() {
            dispatch_commands(
                update::update(&mut model, queued_msg),
                &poller_handles,
                &cmd_client,
                &cmd_tx,
            )
            .await?;
        }

        if model.should_quit {
            for h in &poller_handles {
                h.abort();
            }
            return Ok(());
        }
    }
}

/// Dispatch side-effect commands returned by update().
async fn dispatch_commands(
    commands: Vec<Command>,
    poller_handles: &[tokio::task::JoinHandle<()>],
    client: &Arc<tokio::sync::Mutex<DaemonClient>>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> Result<()> {
    for cmd in commands {
        match cmd {
            Command::Quit => {
                for h in poller_handles {
                    h.abort();
                }
                return Ok(());
            }
            Command::SetFanSpeed(speed) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_fan_speed_percent(speed)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: format!("Set fan speed to {}%", speed),
                        result,
                    }));
                });
            }
            Command::None => {}
            Command::FetchProfiles => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    send_profile_list(&client, &t).await;
                });
            }
            Command::FetchProfileDetail(id) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_profile(&id).await {
                        Ok(json) => {
                            let _ =
                                t.send(Msg::Data(crate::msg::DataUpdate::ProfileDetail(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: format!("Fetch profile {}", id),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::CopyProfile(id) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client.copy_profile(&id).await.map(|_| ()).map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Copy profile".into(),
                        result,
                    }));
                    send_profile_list(&client, &t).await;
                });
            }
            Command::DeleteProfile(id) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client.delete_profile(&id).await.map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Delete profile".into(),
                        result,
                    }));
                    send_profile_list(&client, &t).await;
                    send_assignments(&client, &t).await;
                });
            }
            Command::SaveProfile { id, json } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .update_profile(&id, &json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save profile".into(),
                        result,
                    }));
                    send_profile_list(&client, &t).await;
                });
            }
            Command::SetActiveProfile { id, state } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_active_profile(&id, &state)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: format!("Set {} profile", state),
                        result,
                    }));
                    send_assignments(&client, &t).await;
                });
            }
            Command::FetchAssignments => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    send_assignments(&client, &t).await;
                });
            }
            Command::FetchActiveFanCurve => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    if let Ok(json) = client.get_active_fan_curve().await
                        && let Some((profile_name, fan_profile, curve)) =
                            crate::poller::parse_fan_curve_response(&json)
                    {
                        let _ = t.send(Msg::Data(
                            crate::msg::DataUpdate::FanCurveData {
                                profile_name,
                                fan_profile,
                                curve_cpu: curve,
                            },
                        ));
                    }
                });
            }
            Command::SaveFanCurve(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_fan_curve(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save fan curve".into(),
                        result,
                    }));
                    // Refresh the curve after saving
                    if let Ok(curve_json) = client.get_active_fan_curve().await
                        && let Some((profile_name, fan_profile, curve)) =
                            crate::poller::parse_fan_curve_response(&curve_json)
                    {
                        let _ = t.send(Msg::Data(
                            crate::msg::DataUpdate::FanCurveData {
                                profile_name,
                                fan_profile,
                                curve_cpu: curve,
                            },
                        ));
                    }
                });
            }
            Command::FetchSettings => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_global_settings().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::SettingsData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch settings".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SaveSettings(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_global_settings(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save settings".into(),
                        result,
                    }));
                });
            }
            Command::FetchKeyboard => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_keyboard_state().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::KeyboardData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch keyboard".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SaveKeyboard(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_keyboard_state(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save keyboard settings".into(),
                        result,
                    }));
                });
            }
            Command::FetchCharging => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_charging_settings().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ChargingData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch charging".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SaveCharging(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_charging_settings(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save charging settings".into(),
                        result,
                    }));
                });
            }
            Command::FetchGpuInfo => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_gpu_info().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::GpuData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch GPU info".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::FetchPowerSettings => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_power_settings().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::PowerData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch power settings".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SavePowerSettings(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_power_settings(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save power settings".into(),
                        result,
                    }));
                });
            }
            Command::ScheduleShutdown { hours, minutes } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .schedule_shutdown(hours, minutes)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: format!("Schedule shutdown in {}h {}m", hours, minutes),
                        result,
                    }));
                });
            }
            Command::CancelShutdown => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .cancel_shutdown()
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Cancel shutdown".into(),
                        result,
                    }));
                });
            }
            Command::FetchDisplay => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_display_modes().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::DisplayData(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch display settings".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SaveDisplay(json) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_display_settings(&json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save display settings".into(),
                        result,
                    }));
                });
            }
            Command::FetchWebcamDevices => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.list_webcam_devices().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::WebcamDevices(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch webcam devices".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::FetchWebcamControls(device) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_webcam_controls(&device).await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::WebcamControls(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch webcam controls".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
            Command::SaveWebcamControls { device, json } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    let result = client
                        .set_webcam_controls(&device, &json)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                        action: "Save webcam controls".into(),
                        result,
                    }));
                });
            }
            Command::FetchSystemInfo => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let client = c.lock().await;
                    match client.get_system_info().await {
                        Ok(json) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::SystemInfo(json)));
                        }
                        Err(e) => {
                            let _ = t.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                                action: "Fetch system info".into(),
                                result: Err(e.to_string()),
                            }));
                        }
                    }
                });
            }
        }
    }
    Ok(())
}

/// Fetch profile list from daemon and send to TUI.
async fn send_profile_list(
    client: &DaemonClient,
    tx: &mpsc::UnboundedSender<Msg>,
) {
    match client.list_profiles().await {
        Ok(json) => {
            if let Ok(profiles) =
                serde_json::from_str::<Vec<crate::msg::ProfileSummary>>(&json)
            {
                let _ = tx.send(Msg::Data(crate::msg::DataUpdate::ProfileList(profiles)));
            }
        }
        Err(e) => {
            let _ = tx.send(Msg::Data(crate::msg::DataUpdate::ActionResult {
                action: "Fetch profiles".into(),
                result: Err(e.to_string()),
            }));
        }
    }
}

/// Fetch profile assignments from daemon and send to TUI.
async fn send_assignments(
    client: &DaemonClient,
    tx: &mpsc::UnboundedSender<Msg>,
) {
    if let Ok(json) = client.get_profile_assignments().await
        && let Ok(settings) = serde_json::from_str::<serde_json::Value>(&json)
    {
        let ac = settings
            .get("stateMap")
            .and_then(|m| m.get("power_ac"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let bat = settings
            .get("stateMap")
            .and_then(|m| m.get("power_bat"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let _ = tx.send(Msg::Data(
            crate::msg::DataUpdate::ProfileAssignments { ac, bat },
        ));
    }
}
