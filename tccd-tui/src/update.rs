use crossterm::event::{KeyCode, KeyModifiers};

use crate::command::Command;
use crate::model::{Model, ProfileView, Tab};
use crate::msg::{DataUpdate, Msg};

/// Pure state transition — no I/O. Returns updated model and side-effect commands.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Command> {
    let mut commands = Vec::new();

    match msg {
        Msg::Key(key) => {
            // Help overlay intercepts all keys when visible
            if model.help_visible {
                match key.code {
                    KeyCode::Char('?') | KeyCode::Esc => {
                        model.help_visible = false;
                        model.dirty = true;
                    }
                    _ => {}
                }
                return commands;
            }

            // When a text field has focus, give it priority over hotkeys
            // for character input and backspace. Ctrl/Alt combos bypass this
            // so shortcuts like Ctrl+S still work.
            if matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace)
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                let (is_text, consumed) = {
                    let form_opt = if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List
                    {
                        model.profiles.editor_form.as_mut()
                    } else {
                        match model.active_tab {
                            Tab::Settings => model.settings.form.as_mut(),
                            Tab::Keyboard => model.keyboard.form.as_mut(),
                            Tab::Charging => model.charging.form.as_mut(),
                            Tab::Power => model.power.form.as_mut(),
                            Tab::Display => model.display.form.as_mut(),
                            Tab::Webcam => model.webcam.form.as_mut(),
                            _ => None,
                        }
                    };
                    match form_opt {
                        Some(form) if form.is_text_focused() => {
                            (true, form.handle_key(key.code))
                        }
                        _ => (false, false),
                    }
                };
                if is_text {
                    if consumed {
                        if model.active_tab == Tab::Profiles {
                            model.profiles.editor_dirty = model
                                .profiles
                                .editor_form
                                .as_ref()
                                .is_some_and(|f| f.is_dirty());
                        }
                        model.dirty = true;
                    }
                    return commands;
                }
            }

            match (key.modifiers, key.code) {
                // '?' toggles help overlay
                (_, KeyCode::Char('?')) => {
                    model.help_visible = true;
                    model.dirty = true;
                }
                (_, KeyCode::Char('q')) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    // In editor, q goes back to list rather than quitting
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List
                    {
                        model.profiles.view = ProfileView::List;
                        model.profiles.editing_json = None;
                        model.profiles.editor_form = None;
                        model.profiles.editor_dirty = false;
                        model.dirty = true;
                    } else if matches!(
                        model.active_tab,
                        Tab::Settings | Tab::Keyboard | Tab::Charging | Tab::Power | Tab::Display | Tab::Webcam
                    ) && key.code == KeyCode::Char('q')
                    {
                        // On form tabs, 'q' quits only when no form is active
                        // (Ctrl+C always quits)
                        if key.modifiers != KeyModifiers::CONTROL {
                            model.should_quit = true;
                            commands.push(Command::Quit);
                        }
                    } else if key.modifiers == KeyModifiers::CONTROL
                        || model.active_tab != Tab::Profiles
                        || model.profiles.view == ProfileView::List
                    {
                        model.should_quit = true;
                        commands.push(Command::Quit);
                    }
                }
                (_, KeyCode::Char('1')) => {
                    model.active_tab = Tab::Dashboard;
                    model.dirty = true;
                }
                (_, KeyCode::Char('2')) => {
                    model.active_tab = Tab::Profiles;
                    model.dirty = true;
                    // Fetch profiles when switching to tab
                    commands.push(Command::FetchProfiles);
                    commands.push(Command::FetchAssignments);
                }
                (_, KeyCode::Char('3')) => {
                    model.active_tab = Tab::FanCurve;
                    model.dirty = true;
                    commands.push(Command::FetchActiveFanCurve);
                }
                (_, KeyCode::Char('4')) => {
                    model.active_tab = Tab::Settings;
                    model.dirty = true;
                    if !model.settings.loaded {
                        commands.push(Command::FetchSettings);
                    }
                }
                (_, KeyCode::Char('5')) => {
                    model.active_tab = Tab::Keyboard;
                    model.dirty = true;
                    if !model.keyboard.loaded {
                        commands.push(Command::FetchKeyboard);
                    }
                }
                (_, KeyCode::Char('6')) => {
                    model.active_tab = Tab::Charging;
                    model.dirty = true;
                    if !model.charging.loaded {
                        commands.push(Command::FetchCharging);
                    }
                }
                (_, KeyCode::Char('7')) => {
                    model.active_tab = Tab::Power;
                    model.dirty = true;
                    if !model.power.loaded {
                        commands.push(Command::FetchGpuInfo);
                        commands.push(Command::FetchPowerSettings);
                    }
                }
                (_, KeyCode::Char('8')) => {
                    model.active_tab = Tab::Display;
                    model.dirty = true;
                    if !model.display.loaded {
                        commands.push(Command::FetchDisplay);
                    }
                }
                (_, KeyCode::Char('9')) => {
                    model.active_tab = Tab::Webcam;
                    model.dirty = true;
                    if !model.webcam.loaded {
                        commands.push(Command::FetchWebcamDevices);
                    }
                }
                (_, KeyCode::Char('0')) => {
                    model.active_tab = Tab::Info;
                    model.dirty = true;
                    if !model.info.loaded {
                        commands.push(Command::FetchSystemInfo);
                    }
                }
                // Profile list navigation
                (_, KeyCode::Up | KeyCode::Char('k'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if model.profiles.selected_index > 0 {
                        model.profiles.selected_index -= 1;
                        model.dirty = true;
                    }
                }
                (_, KeyCode::Down | KeyCode::Char('j'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if model.profiles.selected_index + 1 < model.profiles.profiles.len() {
                        model.profiles.selected_index += 1;
                        model.dirty = true;
                    }
                }
                // Enter — open profile editor
                (_, KeyCode::Enter)
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if let Some(p) = model.profiles.profiles.get(model.profiles.selected_index) {
                        let id = p.id.clone();
                        model.profiles.view = ProfileView::Editor {
                            profile_id: id.clone(),
                        };
                        model.profiles.editor_dirty = false;
                        model.dirty = true;
                        commands.push(Command::FetchProfileDetail(id));
                    }
                }
                // 'c' — copy profile
                (_, KeyCode::Char('c'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if let Some(p) = model.profiles.profiles.get(model.profiles.selected_index) {
                        commands.push(Command::CopyProfile(p.id.clone()));
                    }
                }
                // 'd' — delete profile (only custom)
                (_, KeyCode::Char('d'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if let Some(p) = model.profiles.profiles.get(model.profiles.selected_index) {
                        if !is_default_profile(&p.id) {
                            commands.push(Command::DeleteProfile(p.id.clone()));
                        } else {
                            model.push_notification(crate::model::Notification {
                                message: "Cannot delete default profiles".into(),
                                is_error: true,
                            });
                            model.dirty = true;
                        }
                    }
                }
                // 'a' — assign to AC
                (_, KeyCode::Char('a'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if let Some(p) = model.profiles.profiles.get(model.profiles.selected_index) {
                        commands.push(Command::SetActiveProfile {
                            id: p.id.clone(),
                            state: "power_ac".into(),
                        });
                    }
                }
                // 'b' — assign to Battery
                (_, KeyCode::Char('b'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view == ProfileView::List =>
                {
                    if let Some(p) = model.profiles.profiles.get(model.profiles.selected_index) {
                        commands.push(Command::SetActiveProfile {
                            id: p.id.clone(),
                            state: "power_bat".into(),
                        });
                    }
                }
                // Editor navigation
                (_, KeyCode::Up | KeyCode::Char('k'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List =>
                {
                    if let Some(ref mut form) = model.profiles.editor_form {
                        form.handle_key(KeyCode::Up);
                        model.profiles.editor_dirty = form.is_dirty();
                        model.dirty = true;
                    }
                }
                (_, KeyCode::Down | KeyCode::Char('j'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List =>
                {
                    if let Some(ref mut form) = model.profiles.editor_form {
                        form.handle_key(KeyCode::Down);
                        model.profiles.editor_dirty = form.is_dirty();
                        model.dirty = true;
                    }
                }
                // Profile editor: save with 's'
                (_, KeyCode::Char('s'))
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List =>
                {
                    if let ProfileView::Editor { ref profile_id } = model.profiles.view
                        && !is_default_profile(profile_id)
                        && let Some(ref form) = model.profiles.editor_form
                        && let Some(ref original_json) = model.profiles.editing_json
                    {
                        let json = apply_profile_form(form, original_json);
                        let cmd = Command::SaveProfile {
                            id: profile_id.clone(),
                            json,
                        };
                        commands.push(cmd);
                        model.profiles.editor_dirty = false;
                        model.dirty = true;
                    }
                }
                // Esc — back to list from editor (must be before the catch-all)
                (_, KeyCode::Esc) if model.active_tab == Tab::Profiles => {
                    if model.profiles.view != ProfileView::List {
                        model.profiles.view = ProfileView::List;
                        model.profiles.editing_json = None;
                        model.profiles.editor_form = None;
                        model.profiles.editor_dirty = false;
                        model.dirty = true;
                    }
                }
                // Profile editor: delegate other keys to form widget
                (_, code)
                    if model.active_tab == Tab::Profiles
                        && model.profiles.view != ProfileView::List =>
                {
                    if let Some(ref mut form) = model.profiles.editor_form
                        && form.handle_key(code)
                    {
                        model.profiles.editor_dirty = form.is_dirty();
                        model.dirty = true;
                    }
                }
                // Esc — discard changes on form tabs
                (_, KeyCode::Esc)
                    if matches!(
                        model.active_tab,
                        Tab::Settings | Tab::Keyboard | Tab::Charging | Tab::Power | Tab::Display | Tab::Webcam
                    ) =>
                {
                    discard_form_changes(model);
                    model.dirty = true;
                }
                // 's' — save on form tabs
                (_, KeyCode::Char('s'))
                    if matches!(
                        model.active_tab,
                        Tab::Settings | Tab::Keyboard | Tab::Charging | Tab::Power | Tab::Display | Tab::Webcam
                    ) =>
                {
                    if let Some(cmd) = save_form(model) {
                        commands.push(cmd);
                    }
                }
                // Webcam: Tab/Backtab switch devices
                (_, KeyCode::Tab) if model.active_tab == Tab::Webcam => {
                    if !model.webcam.devices.is_empty() {
                        model.webcam.selected_device =
                            (model.webcam.selected_device + 1) % model.webcam.devices.len();
                        if let Some(dev) = model.webcam.devices.get(model.webcam.selected_device) {
                            commands.push(Command::FetchWebcamControls(dev.path.clone()));
                        }
                        model.webcam.form = None;
                        model.dirty = true;
                    }
                }
                (_, KeyCode::BackTab) if model.active_tab == Tab::Webcam => {
                    if !model.webcam.devices.is_empty() {
                        model.webcam.selected_device = if model.webcam.selected_device == 0 {
                            model.webcam.devices.len() - 1
                        } else {
                            model.webcam.selected_device - 1
                        };
                        if let Some(dev) = model.webcam.devices.get(model.webcam.selected_device) {
                            commands.push(Command::FetchWebcamControls(dev.path.clone()));
                        }
                        model.webcam.form = None;
                        model.dirty = true;
                    }
                }
                // Form field input on form tabs (delegate to FormState)
                (_, code)
                    if matches!(
                        model.active_tab,
                        Tab::Settings | Tab::Keyboard | Tab::Charging | Tab::Power | Tab::Display | Tab::Webcam
                    ) =>
                {
                    let changed = match model.active_tab {
                        Tab::Settings => model
                            .settings
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        Tab::Keyboard => model
                            .keyboard
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        Tab::Charging => model
                            .charging
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        Tab::Power => model
                            .power
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        Tab::Display => model
                            .display
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        Tab::Webcam => model
                            .webcam
                            .form
                            .as_mut()
                            .is_some_and(|f| f.handle_key(code)),
                        _ => false,
                    };
                    if changed {
                        model.dirty = true;
                    }
                }
                // Fan curve point navigation
                (_, KeyCode::Left)
                    if model.active_tab == Tab::FanCurve =>
                {
                    if model.fan_curve.selected_point > 0 {
                        model.fan_curve.selected_point -= 1;
                        model.dirty = true;
                    }
                }
                (_, KeyCode::Right)
                    if model.active_tab == Tab::FanCurve =>
                {
                    if model.fan_curve.selected_point + 1
                        < model.fan_curve.curve_points.len()
                    {
                        model.fan_curve.selected_point += 1;
                        model.dirty = true;
                    }
                }
                // Fan curve: adjust speed at selected point
                (_, KeyCode::Up)
                    if model.active_tab == Tab::FanCurve =>
                {
                    if let Some(point) = model.fan_curve.curve_points.get_mut(model.fan_curve.selected_point) {
                        point.1 = (point.1 + 5.0).min(100.0);
                        model.dirty = true;
                    }
                }
                (_, KeyCode::Down)
                    if model.active_tab == Tab::FanCurve =>
                {
                    if let Some(point) = model.fan_curve.curve_points.get_mut(model.fan_curve.selected_point) {
                        point.1 = (point.1 - 5.0).max(0.0);
                        model.dirty = true;
                    }
                }
                // Fan curve: insert new point after selected
                (_, KeyCode::Insert | KeyCode::Char('i'))
                    if model.active_tab == Tab::FanCurve =>
                {
                    let idx = model.fan_curve.selected_point;
                    if let Some(&current) = model.fan_curve.curve_points.get(idx) {
                        let next = model.fan_curve.curve_points.get(idx + 1).copied();
                        let new_point = match next {
                            Some(n) => (((current.0 + n.0) / 2.0).round(), ((current.1 + n.1) / 2.0).round()),
                            None => (((current.0 + 100.0) / 2.0).round(), ((current.1 + 100.0) / 2.0).round()),
                        };
                        model.fan_curve.curve_points.insert(idx + 1, new_point);
                        model.fan_curve.selected_point = idx + 1;
                        model.dirty = true;
                    }
                }
                // Fan curve: delete selected point (min 2, protect first/last)
                (_, KeyCode::Delete | KeyCode::Char('x'))
                    if model.active_tab == Tab::FanCurve =>
                {
                    let idx = model.fan_curve.selected_point;
                    let len = model.fan_curve.curve_points.len();
                    if len > 2 && idx > 0 && idx < len - 1 {
                        model.fan_curve.curve_points.remove(idx);
                        if model.fan_curve.selected_point >= model.fan_curve.curve_points.len() {
                            model.fan_curve.selected_point = model.fan_curve.curve_points.len() - 1;
                        }
                        model.dirty = true;
                    }
                }
                // Fan curve: reset to default 5-point curve
                (_, KeyCode::Char('r'))
                    if model.active_tab == Tab::FanCurve =>
                {
                    model.fan_curve.curve_points = default_fan_curve();
                    model.fan_curve.selected_point = 0;
                    model.dirty = true;
                }
                // Fan curve: save
                (_, KeyCode::Char('s'))
                    if model.active_tab == Tab::FanCurve =>
                {
                    if model.fan_curve.is_dirty() {
                        let json = fan_curve_to_json(&model.fan_curve.curve_points);
                        model.fan_curve.original_points = model.fan_curve.curve_points.clone();
                        model.dirty = true;
                        commands.push(Command::SaveFanCurve(json));
                    }
                }
                // Fan curve: discard changes
                (_, KeyCode::Esc)
                    if model.active_tab == Tab::FanCurve =>
                {
                    if model.fan_curve.is_dirty() {
                        model.fan_curve.curve_points = model.fan_curve.original_points.clone();
                        model.fan_curve.selected_point = model.fan_curve.selected_point
                            .min(model.fan_curve.curve_points.len().saturating_sub(1));
                        model.dirty = true;
                    }
                }
                _ => {}
            }
        }
        Msg::Resize(w, h) => {
            if w < 80 || h < 24 {
                model.push_notification(crate::model::Notification {
                    message: format!("Terminal too small ({}x{}, need 80x24)", w, h),
                    is_error: true,
                });
            }
            model.dirty = true;
        }
        Msg::Tick => {}
        Msg::Data(data_update) => {
            handle_data(model, data_update, &mut commands);
        }
    }

    commands
}

fn handle_data(model: &mut Model, data_update: DataUpdate, commands: &mut Vec<Command>) {
    match data_update {
        DataUpdate::FanData(fan) => {
            if let Some(&speed) = fan.speeds_percent.first() {
                model.dashboard.push_fan_speed(speed as f64);
            }
            model.dashboard.fan = fan;
            model.dirty = true;
        }
        DataUpdate::CpuMetrics(cpu) => {
            if let Some(temp) = cpu.temperature {
                model.dashboard.push_cpu_temp(temp);
            }
            model.dashboard.cpu = cpu;
            model.dirty = true;
        }
        DataUpdate::ConnectionLost => {
            model.connection_status = crate::model::ConnectionStatus::Disconnected;
            model.push_notification(crate::model::Notification {
                message: "D-Bus connection lost".into(),
                is_error: true,
            });
            model.dirty = true;
        }
        DataUpdate::ConnectionRestored => {
            model.connection_status = crate::model::ConnectionStatus::Connected;
            model.push_notification(crate::model::Notification {
                message: "D-Bus connection restored".into(),
                is_error: false,
            });
            model.dirty = true;
        }
        DataUpdate::ActionResult { action, result } => {
            let notification = match result {
                Ok(()) => crate::model::Notification {
                    message: format!("{action}: OK"),
                    is_error: false,
                },
                Err(e) => crate::model::Notification {
                    message: format!("{action}: {e}"),
                    is_error: true,
                },
            };
            model.push_notification(notification);
            model.dirty = true;
        }
        DataUpdate::ProfileList(profiles) => {
            model.profiles.profiles = profiles;
            // Clamp selection
            if model.profiles.selected_index >= model.profiles.profiles.len() {
                model.profiles.selected_index =
                    model.profiles.profiles.len().saturating_sub(1);
            }
            model.dirty = true;
        }
        DataUpdate::ProfileDetail(json) => {
            model.profiles.editing_json = Some(json.clone());
            let is_default = match &model.profiles.view {
                ProfileView::Editor { profile_id } => is_default_profile(profile_id),
                _ => false,
            };
            model.profiles.editor_form = Some(parse_profile_form(&json, is_default));
            model.dirty = true;
        }
        DataUpdate::ProfileAssignments { ac, bat } => {
            model.profiles.ac_profile_id = ac;
            model.profiles.bat_profile_id = bat;
            model.dirty = true;
        }
        DataUpdate::PowerState { on_ac } => {
            model.dashboard.power_on_ac = Some(on_ac);
            model.dirty = true;
        }
        DataUpdate::FanCurveData {
            profile_name,
            fan_profile,
            curve_cpu,
        } => {
            model.dashboard.active_profile_name = Some(profile_name);
            model.fan_curve.fan_profile_name = fan_profile;
            // Don't overwrite in-progress edits
            if !model.fan_curve.is_dirty() {
                model.fan_curve.curve_points = curve_cpu.clone();
                if model.fan_curve.selected_point >= model.fan_curve.curve_points.len() {
                    model.fan_curve.selected_point =
                        model.fan_curve.curve_points.len().saturating_sub(1);
                }
            }
            model.fan_curve.original_points = curve_cpu;
            model.dirty = true;
        }
        DataUpdate::SettingsData(json) => {
            model.settings.form = Some(parse_settings_form(&json));
            model.settings.loaded = true;
            model.dirty = true;
        }
        DataUpdate::KeyboardData(json) => {
            model.keyboard.form = Some(parse_keyboard_form(&json));
            model.keyboard.loaded = true;
            model.dirty = true;
        }
        DataUpdate::ChargingData(json) => {
            model.charging.form = Some(parse_charging_form(&json, &model.capabilities));
            model.charging.loaded = true;
            model.dirty = true;
        }
        DataUpdate::GpuData(json) => {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            model.power.gpu_info = Some(crate::model::GpuInfo {
                dgpu_name: v.get("dgpuName").and_then(|v| v.as_str()).unwrap_or("").into(),
                dgpu_temp: v.get("dgpuTemp").and_then(|v| v.as_f64()),
                dgpu_usage: v.get("dgpuUsage").and_then(|v| v.as_f64()),
                dgpu_power_draw: v.get("dgpuPowerDraw").and_then(|v| v.as_f64()),
                igpu_name: v.get("igpuName").and_then(|v| v.as_str()).unwrap_or("").into(),
                igpu_usage: v.get("igpuUsage").and_then(|v| v.as_f64()),
                prime_mode: v.get("primeMode").and_then(|v| v.as_str()).unwrap_or("").into(),
                tgp_offset: v.get("tgpOffset").and_then(|v| v.as_f64()),
            });
            model.dirty = true;
        }
        DataUpdate::PowerData(json) => {
            model.power.form = Some(parse_power_form(&json));
            model.power.loaded = true;
            model.dirty = true;
        }
        DataUpdate::DisplayData(json) => {
            model.display.form = Some(parse_display_form(&json));
            model.display.loaded = true;
            // Store original JSON for display_form_to_json (needs refresh_rates/resolutions lists)
            model.display.original_json = Some(json);
            model.dirty = true;
        }
        DataUpdate::WebcamDevices(json) => {
            if let Ok(devices) = serde_json::from_str::<Vec<serde_json::Value>>(&json) {
                model.webcam.devices = devices
                    .iter()
                    .map(|d| crate::model::WebcamDevice {
                        path: d.get("path").and_then(|v| v.as_str()).unwrap_or("").into(),
                        name: d.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").into(),
                    })
                    .collect();
                model.webcam.selected_device = 0;
                model.webcam.loaded = true;
                // Fetch controls for first device
                if let Some(dev) = model.webcam.devices.first() {
                    commands.push(Command::FetchWebcamControls(dev.path.clone()));
                }
            }
            model.dirty = true;
        }
        DataUpdate::WebcamControls(json) => {
            model.webcam.form = Some(parse_webcam_form(&json));
            model.dirty = true;
        }
        DataUpdate::SystemInfo(json) => {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            model.info.tcc_version = v.get("tccVersion").and_then(|v| v.as_str()).map(String::from);
            model.info.daemon_version = v.get("daemonVersion").and_then(|v| v.as_str()).map(String::from);
            model.info.hostname = v.get("hostname").and_then(|v| v.as_str()).map(String::from);
            model.info.kernel_version = v.get("kernelVersion").and_then(|v| v.as_str()).map(String::from);
            model.info.loaded = true;
            model.dirty = true;
        }
        DataUpdate::Capabilities(json) => {
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            model.capabilities.charge_thresholds = v.get("chargeThresholds").and_then(|v| v.as_bool()).unwrap_or(false);
            model.capabilities.charging_profile = v.get("chargingProfile").and_then(|v| v.as_bool()).unwrap_or(false);
            model.capabilities.fan_control = v.get("fanControl").and_then(|v| v.as_bool()).unwrap_or(false);
            model.capabilities.display_brightness = v.get("displayBrightness").and_then(|v| v.as_bool()).unwrap_or(false);
            // Rebuild forms if they were already loaded, so fields reflect capabilities
            if model.charging.loaded {
                commands.push(Command::FetchCharging);
            }
            model.dirty = true;
        }
    }
}

fn is_default_profile(id: &str) -> bool {
    id.starts_with("__") && id.ends_with("__")
}

// ─── Shared option constants ────────────────────────────────────────

use crate::widgets::form::{FormField, FormState};

const GOVERNORS: &[&str] = &["powersave", "performance", "schedutil", "ondemand", "conservative"];
const ENERGY_PERF_PREFS: &[&str] = &["power", "balance_power", "balance_performance", "performance"];
const FAN_PROFILES: &[&str] = &["Silent", "Quiet", "Balanced", "Enthusiast", "Overboost"];
const KEYBOARD_MODES: &[&str] = &["Static", "Breathing", "Wave", "Color Cycle"];
const CHARGING_PROFILES: &[&str] = &["Full Capacity", "Reduced", "Stationary"];
const CHARGING_PRIORITIES: &[&str] = &["Battery", "Performance"];
const PRIME_MODES: &[&str] = &["on-demand", "performance", "offload", "integrated"];

// ─── Settings form parsing ──────────────────────────────────────────

fn parse_settings_form(json: &str) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let fahrenheit = v.get("fahrenheit").and_then(|v| v.as_bool()).unwrap_or(false);
    let cpu_enabled = v
        .get("cpuSettingsEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let fan_enabled = v
        .get("fanControlEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let fields = vec![
        FormField::section("── Temperature ──"),
        FormField::toggle("Use Fahrenheit", fahrenheit),
        FormField::section("── Feature Toggles ──"),
        FormField::toggle("CPU Settings", cpu_enabled),
        FormField::toggle("Fan Control", fan_enabled),
    ];
    FormState::new(fields)
}

fn settings_form_to_json(form: &FormState) -> String {
    let fahrenheit = form.field_by_label("Use Fahrenheit").and_then(|f| f.value.as_bool()).unwrap_or(false);
    let cpu_enabled = form.field_by_label("CPU Settings").and_then(|f| f.value.as_bool()).unwrap_or(true);
    let fan_enabled = form.field_by_label("Fan Control").and_then(|f| f.value.as_bool()).unwrap_or(true);

    serde_json::json!({
        "fahrenheit": fahrenheit,
        "cpuSettingsEnabled": cpu_enabled,
        "fanControlEnabled": fan_enabled,
    })
    .to_string()
}

// ─── Keyboard form parsing ──────────────────────────────────────────

fn parse_keyboard_form(json: &str) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let brightness = v
        .get("brightness")
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0);
    let color = v
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("#ffffff")
        .to_string();

    let mode_options: Vec<String> = KEYBOARD_MODES.iter().map(|s| (*s).into()).collect();
    let mode_str = v.get("mode").and_then(|v| v.as_str()).unwrap_or("Static");
    let mode_idx = KEYBOARD_MODES
        .iter()
        .position(|m| *m == mode_str)
        .unwrap_or(0);

    let fields = vec![
        FormField::section("── Backlight ──"),
        FormField::number("Brightness", brightness, 0.0, 100.0, 5.0),
        FormField::text("Color (hex)", color, 7),
        FormField::select("Mode", mode_options, mode_idx),
    ];
    FormState::new(fields)
}

fn keyboard_form_to_json(form: &FormState) -> String {
    let brightness = form.field_by_label("Brightness").and_then(|f| f.value.as_number()).unwrap_or(50.0);
    let color = form
        .field_by_label("Color (hex)")
        .and_then(|f| f.value.as_text())
        .unwrap_or("#ffffff");

    let mode_idx = form.field_by_label("Mode").and_then(|f| f.value.as_select()).unwrap_or(0);
    let mode = KEYBOARD_MODES.get(mode_idx).unwrap_or(&"Static");

    serde_json::json!({
        "brightness": brightness,
        "color": color,
        "mode": mode,
    })
    .to_string()
}

// ─── Charging form parsing ──────────────────────────────────────────

fn parse_charging_form(json: &str, caps: &crate::model::Capabilities) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let profile_options: Vec<String> = CHARGING_PROFILES.iter().map(|s| (*s).into()).collect();
    let profile_str = v
        .get("chargingProfile")
        .and_then(|v| v.as_str())
        .unwrap_or("Full Capacity");
    let profile_idx = CHARGING_PROFILES
        .iter()
        .position(|p| *p == profile_str)
        .unwrap_or(0);

    let priority_options: Vec<String> = CHARGING_PRIORITIES.iter().map(|s| (*s).into()).collect();
    let priority_str = v
        .get("chargingPriority")
        .and_then(|v| v.as_str())
        .unwrap_or("Battery");
    let priority_idx = CHARGING_PRIORITIES
        .iter()
        .position(|p| *p == priority_str)
        .unwrap_or(0);

    let start_threshold = v
        .get("startThreshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(80.0);
    let end_threshold = v
        .get("endThreshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0);

    let fields = vec![
        FormField::section("── Charging Profile ──"),
        if caps.charging_profile {
            FormField::select("Profile", profile_options, profile_idx)
        } else {
            FormField::read_only("Profile", "N/A (hardware unsupported)")
        },
        if caps.charging_profile {
            FormField::select("Priority", priority_options, priority_idx)
        } else {
            FormField::read_only("Priority", "N/A (hardware unsupported)")
        },
        FormField::section("── Thresholds ──"),
        if caps.charge_thresholds {
            FormField::number("Start (%)", start_threshold, 20.0, 100.0, 5.0)
        } else {
            FormField::read_only("Start (%)", "N/A (hardware unsupported)")
        },
        if caps.charge_thresholds {
            FormField::number("End (%)", end_threshold, 20.0, 100.0, 5.0)
        } else {
            FormField::read_only("End (%)", "N/A (hardware unsupported)")
        },
    ];
    FormState::new(fields)
}

fn charging_form_to_json(form: &FormState) -> String {
    let profile_field = form.field_by_label("Profile");
    let profile = profile_field
        .and_then(|f| f.value.as_select())
        .and_then(|i| CHARGING_PROFILES.get(i).copied())
        .unwrap_or("Full Capacity");

    let priority_field = form.field_by_label("Priority");
    let priority = priority_field
        .and_then(|f| f.value.as_select())
        .and_then(|i| CHARGING_PRIORITIES.get(i).copied())
        .unwrap_or("Battery");

    let start = form.field_by_label("Start (%)").and_then(|f| f.value.as_number()).unwrap_or(80.0);
    let end = form.field_by_label("End (%)").and_then(|f| f.value.as_number()).unwrap_or(100.0);

    serde_json::json!({
        "chargingProfile": profile,
        "chargingPriority": priority,
        "startThreshold": start,
        "endThreshold": end,
    })
    .to_string()
}

// ─── Power form parsing ─────────────────────────────────────────────

fn parse_power_form(json: &str) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let prime_options: Vec<String> = PRIME_MODES.iter().map(|s| (*s).into()).collect();
    let prime_str = v
        .get("primeMode")
        .and_then(|v| v.as_str())
        .unwrap_or("on-demand");
    let prime_idx = PRIME_MODES
        .iter()
        .position(|m| *m == prime_str)
        .unwrap_or(0);

    let tgp_offset = v
        .get("tgpOffset")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let shutdown_hours = v
        .get("shutdownHours")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let shutdown_minutes = v
        .get("shutdownMinutes")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let shutdown_active = v
        .get("shutdownActive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let fields = vec![
        FormField::section("── PRIME Mode ──"),
        FormField::select("PRIME Mode", prime_options, prime_idx),
        FormField::section("── TGP ──"),
        FormField::number("TGP Offset (W)", tgp_offset, -50.0, 50.0, 5.0),
        FormField::section("── Scheduled Shutdown ──"),
        FormField::toggle("Shutdown Active", shutdown_active),
        FormField::number("Hours", shutdown_hours, 0.0, 24.0, 1.0),
        FormField::number("Minutes", shutdown_minutes, 0.0, 59.0, 5.0),
    ];
    FormState::new(fields)
}

fn power_form_to_json(form: &FormState) -> String {
    let prime_idx = form
        .field_by_label("PRIME Mode")
        .and_then(|f| f.value.as_select())
        .unwrap_or(0);
    let prime = PRIME_MODES.get(prime_idx).unwrap_or(&"on-demand");

    let tgp_offset = form
        .field_by_label("TGP Offset (W)")
        .and_then(|f| f.value.as_number())
        .unwrap_or(0.0);

    let shutdown_active = form
        .field_by_label("Shutdown Active")
        .and_then(|f| f.value.as_bool())
        .unwrap_or(false);
    let hours = form
        .field_by_label("Hours")
        .and_then(|f| f.value.as_number())
        .unwrap_or(0.0);
    let minutes = form
        .field_by_label("Minutes")
        .and_then(|f| f.value.as_number())
        .unwrap_or(0.0);

    serde_json::json!({
        "primeMode": prime,
        "tgpOffset": tgp_offset,
        "shutdownHours": hours as u32,
        "shutdownMinutes": minutes as u32,
        "shutdownActive": shutdown_active,
    })
    .to_string()
}

// ─── Display form parsing ───────────────────────────────────────────

fn parse_display_form(json: &str) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let brightness = v.get("brightness").and_then(|v| v.as_f64()).unwrap_or(80.0);

    let refresh_rates: Vec<u32> = v
        .get("refreshRates")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|r| r.as_u64().map(|n| n as u32)).collect())
        .unwrap_or_default();
    let refresh_options: Vec<String> = refresh_rates.iter().map(|r| format!("{} Hz", r)).collect();
    let selected_rate = v
        .get("selectedRefreshRate")
        .and_then(|v| v.as_u64())
        .unwrap_or(60) as u32;
    let rate_idx = refresh_rates
        .iter()
        .position(|r| *r == selected_rate)
        .unwrap_or(0);

    let resolutions: Vec<String> = v
        .get("resolutions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let selected_res = v
        .get("selectedResolution")
        .and_then(|v| v.as_str())
        .unwrap_or("1920x1080");
    let res_idx = resolutions
        .iter()
        .position(|r| r == selected_res)
        .unwrap_or(0);
    let res_options = resolutions.clone();

    let ycbcr = v.get("ycbcr").and_then(|v| v.as_bool()).unwrap_or(false);

    let fields = vec![
        FormField::section("── Brightness ──"),
        FormField::number("Brightness", brightness, 0.0, 100.0, 5.0),
        FormField::section("── Refresh Rate ──"),
        FormField::select("Refresh Rate", refresh_options, rate_idx),
        FormField::section("── Resolution ──"),
        FormField::select("Resolution", res_options, res_idx),
        FormField::section("── Color ──"),
        FormField::toggle("YCbCr 4:2:0", ycbcr),
    ];
    FormState::new(fields)
}

fn display_form_to_json(form: &FormState, original_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(original_json).unwrap_or_default();

    let brightness = form
        .field_by_label("Brightness")
        .and_then(|f| f.value.as_number())
        .unwrap_or(80.0);

    let refresh_rates: Vec<u32> = v
        .get("refreshRates")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|r| r.as_u64().map(|n| n as u32)).collect())
        .unwrap_or_default();
    let rate_idx = form
        .field_by_label("Refresh Rate")
        .and_then(|f| f.value.as_select())
        .unwrap_or(0);
    let selected_rate = refresh_rates.get(rate_idx).copied().unwrap_or(60);

    let resolutions: Vec<String> = v
        .get("resolutions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let res_idx = form
        .field_by_label("Resolution")
        .and_then(|f| f.value.as_select())
        .unwrap_or(0);
    let selected_res = resolutions
        .get(res_idx)
        .cloned()
        .unwrap_or_else(|| "1920x1080".into());

    let ycbcr = form
        .field_by_label("YCbCr 4:2:0")
        .and_then(|f| f.value.as_bool())
        .unwrap_or(false);

    serde_json::json!({
        "brightness": brightness,
        "refreshRates": refresh_rates,
        "selectedRefreshRate": selected_rate,
        "resolutions": resolutions,
        "selectedResolution": selected_res,
        "ycbcr": ycbcr,
    })
    .to_string()
}

// ─── Webcam form parsing ────────────────────────────────────────────

fn parse_webcam_form(json: &str) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let brightness = v.get("brightness").and_then(|v| v.as_f64()).unwrap_or(128.0);
    let contrast = v.get("contrast").and_then(|v| v.as_f64()).unwrap_or(128.0);
    let saturation = v.get("saturation").and_then(|v| v.as_f64()).unwrap_or(128.0);
    let sharpness = v.get("sharpness").and_then(|v| v.as_f64()).unwrap_or(128.0);
    let auto_exposure = v.get("autoExposure").and_then(|v| v.as_bool()).unwrap_or(true);
    let exposure = v.get("exposure").and_then(|v| v.as_f64()).unwrap_or(500.0);
    let auto_wb = v
        .get("autoWhiteBalance")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let white_balance = v
        .get("whiteBalance")
        .and_then(|v| v.as_f64())
        .unwrap_or(4500.0);

    let fields = vec![
        FormField::section("── Image ──"),
        FormField::number("Brightness", brightness, 0.0, 255.0, 1.0),
        FormField::number("Contrast", contrast, 0.0, 255.0, 1.0),
        FormField::number("Saturation", saturation, 0.0, 255.0, 1.0),
        FormField::number("Sharpness", sharpness, 0.0, 255.0, 1.0),
        FormField::section("── Exposure ──"),
        FormField::toggle("Auto Exposure", auto_exposure),
        FormField::number("Exposure", exposure, 0.0, 10000.0, 50.0),
        FormField::section("── White Balance ──"),
        FormField::toggle("Auto White Balance", auto_wb),
        FormField::number("White Balance", white_balance, 2000.0, 9000.0, 100.0),
    ];
    FormState::new(fields)
}

fn webcam_form_to_json(form: &FormState) -> String {
    let brightness = form.field_by_label("Brightness").and_then(|f| f.value.as_number()).unwrap_or(128.0);
    let contrast = form.field_by_label("Contrast").and_then(|f| f.value.as_number()).unwrap_or(128.0);
    let saturation = form.field_by_label("Saturation").and_then(|f| f.value.as_number()).unwrap_or(128.0);
    let sharpness = form.field_by_label("Sharpness").and_then(|f| f.value.as_number()).unwrap_or(128.0);
    let auto_exposure = form.field_by_label("Auto Exposure").and_then(|f| f.value.as_bool()).unwrap_or(true);
    let exposure = form.field_by_label("Exposure").and_then(|f| f.value.as_number()).unwrap_or(500.0);
    let auto_wb = form.field_by_label("Auto White Balance").and_then(|f| f.value.as_bool()).unwrap_or(true);
    let white_balance = form.field_by_label("White Balance").and_then(|f| f.value.as_number()).unwrap_or(4500.0);

    serde_json::json!({
        "brightness": brightness,
        "contrast": contrast,
        "saturation": saturation,
        "sharpness": sharpness,
        "autoExposure": auto_exposure,
        "exposure": exposure,
        "autoWhiteBalance": auto_wb,
        "whiteBalance": white_balance,
    })
    .to_string()
}

// ─── Form save/discard helpers ──────────────────────────────────────
fn parse_profile_form(json: &str, is_default: bool) -> FormState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();

    let name = v.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let desc = v.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let mut fields = Vec::new();

    if is_default {
        fields.push(FormField::read_only("Name", name));
        fields.push(FormField::read_only("Description", desc));
    } else {
        fields.push(FormField::text("Name", name, 64));
        fields.push(FormField::text("Description", desc, 128));
    }

    // CPU section
    if let Some(cpu) = v.get("cpu") {
        fields.push(FormField::section("── CPU ──"));

        let gov_options: Vec<String> = GOVERNORS.iter().map(|s| (*s).into()).collect();
        let gov_str = cpu.get("governor").and_then(|v| v.as_str()).unwrap_or("powersave");
        let gov_idx = GOVERNORS.iter().position(|g| *g == gov_str).unwrap_or(0);

        let epp_options: Vec<String> = ENERGY_PERF_PREFS.iter().map(|s| (*s).into()).collect();
        let epp_str = cpu
            .get("energyPerformancePreference")
            .and_then(|v| v.as_str())
            .unwrap_or("balance_power");
        let epp_idx = ENERGY_PERF_PREFS.iter().position(|e| *e == epp_str).unwrap_or(1);

        let no_turbo = cpu.get("noTurbo").and_then(|v| v.as_bool()).unwrap_or(false);
        let online_cores = cpu
            .get("onlineCores")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if is_default {
            fields.push(FormField::read_only("Governor", gov_str));
            fields.push(FormField::read_only("Energy Perf Pref", epp_str));
            fields.push(FormField::read_only(
                "No Turbo",
                if no_turbo { "On" } else { "Off" },
            ));
        } else {
            fields.push(FormField::select("Governor", gov_options, gov_idx));
            fields.push(FormField::select("Energy Perf Pref", epp_options, epp_idx));
            fields.push(FormField::toggle("No Turbo", no_turbo));
            if online_cores > 0.0 {
                fields.push(FormField::number("Online Cores", online_cores, 1.0, 128.0, 1.0));
            }
        }
    }

    // Display section
    if let Some(display) = v.get("display") {
        fields.push(FormField::section("── Display ──"));
        let brightness = display
            .get("brightness")
            .and_then(|v| v.as_f64())
            .unwrap_or(50.0);
        let use_brightness = display
            .get("useBrightness")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_default {
            fields.push(FormField::read_only("Brightness", format!("{}", brightness as i32)));
            fields.push(FormField::read_only(
                "Use Brightness",
                if use_brightness { "On" } else { "Off" },
            ));
        } else {
            fields.push(FormField::number("Brightness", brightness, 0.0, 100.0, 5.0));
            fields.push(FormField::toggle("Use Brightness", use_brightness));
        }
    }

    // Fan section
    if let Some(fan) = v.get("fan") {
        fields.push(FormField::section("── Fan ──"));
        let fan_profile_options: Vec<String> = FAN_PROFILES.iter().map(|s| (*s).into()).collect();
        let fp_str = fan.get("fanProfile").and_then(|v| v.as_str()).unwrap_or("Balanced");
        let fp_idx = FAN_PROFILES.iter().position(|f| *f == fp_str).unwrap_or(2);

        let use_control = fan.get("useControl").and_then(|v| v.as_bool()).unwrap_or(true);
        let min_speed = fan.get("minimumFanspeed").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let max_speed = fan.get("maximumFanspeed").and_then(|v| v.as_f64()).unwrap_or(100.0);

        if is_default {
            fields.push(FormField::read_only("Fan Profile", fp_str));
            fields.push(FormField::read_only("Fan Control", if use_control { "On" } else { "Off" }));
            fields.push(FormField::read_only("Min Speed", format!("{}%", min_speed as i32)));
            fields.push(FormField::read_only("Max Speed", format!("{}%", max_speed as i32)));
        } else {
            fields.push(FormField::select("Fan Profile", fan_profile_options, fp_idx));
            fields.push(FormField::toggle("Fan Control", use_control));
            fields.push(FormField::number("Min Speed (%)", min_speed, 0.0, 100.0, 5.0));
            fields.push(FormField::number("Max Speed (%)", max_speed, 0.0, 100.0, 5.0));
        }
    }

    FormState::new(fields)
}

/// Apply form field values back onto the original profile JSON.
fn apply_profile_form(form: &FormState, original_json: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(original_json).unwrap_or_default();

    // Walk through form fields and apply changes by label
    for field in &form.fields {
        match field.label.as_str() {
            "Name" => {
                if let Some(s) = field.value.as_text() {
                    v["name"] = serde_json::Value::String(s.to_string());
                }
            }
            "Description" => {
                if let Some(s) = field.value.as_text() {
                    v["description"] = serde_json::Value::String(s.to_string());
                }
            }
            "Governor" => {
                if let Some(idx) = field.value.as_select()
                    && let Some(gov) = GOVERNORS.get(idx)
                {
                    v["cpu"]["governor"] = serde_json::Value::String(gov.to_string());
                }
            }
            "Energy Perf Pref" => {
                if let Some(idx) = field.value.as_select()
                    && let Some(epp) = ENERGY_PERF_PREFS.get(idx)
                {
                    v["cpu"]["energyPerformancePreference"] =
                        serde_json::Value::String(epp.to_string());
                }
            }
            "No Turbo" => {
                if let Some(b) = field.value.as_bool() {
                    v["cpu"]["noTurbo"] = serde_json::Value::Bool(b);
                }
            }
            "Online Cores" => {
                if let Some(n) = field.value.as_number() {
                    v["cpu"]["onlineCores"] = serde_json::json!(n as u32);
                }
            }
            "Brightness" => {
                if let Some(n) = field.value.as_number() {
                    v["display"]["brightness"] = serde_json::json!(n as i32);
                }
            }
            "Use Brightness" => {
                if let Some(b) = field.value.as_bool() {
                    v["display"]["useBrightness"] = serde_json::Value::Bool(b);
                }
            }
            "Fan Profile" => {
                if let Some(idx) = field.value.as_select()
                    && let Some(fp) = FAN_PROFILES.get(idx)
                {
                    v["fan"]["fanProfile"] = serde_json::Value::String(fp.to_string());
                }
            }
            "Fan Control" => {
                if let Some(b) = field.value.as_bool() {
                    v["fan"]["useControl"] = serde_json::Value::Bool(b);
                }
            }
            "Min Speed (%)" => {
                if let Some(n) = field.value.as_number() {
                    v["fan"]["minimumFanspeed"] = serde_json::json!(n as u8);
                }
            }
            "Max Speed (%)" => {
                if let Some(n) = field.value.as_number() {
                    v["fan"]["maximumFanspeed"] = serde_json::json!(n as u8);
                }
            }
            _ => {}
        }
    }

    serde_json::to_string(&v).unwrap_or_else(|_| original_json.to_string())
}

// ─── Form save/discard helpers ──────────────────────────────────────

fn default_fan_curve() -> Vec<(f64, f64)> {
    vec![
        (0.0, 0.0),
        (40.0, 20.0),
        (60.0, 40.0),
        (80.0, 70.0),
        (100.0, 100.0),
    ]
}

fn fan_curve_to_json(points: &[(f64, f64)]) -> String {
    let entries: Vec<serde_json::Value> = points
        .iter()
        .map(|(temp, speed)| {
            serde_json::json!({
                "temp": *temp as u8,
                "speed": *speed as u8,
            })
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_default()
}

fn discard_form_changes(model: &mut Model) {
    match model.active_tab {
        Tab::Settings => {
            model.settings.loaded = false;
            model.settings.form = None;
        }
        Tab::Keyboard => {
            model.keyboard.loaded = false;
            model.keyboard.form = None;
        }
        Tab::Charging => {
            model.charging.loaded = false;
            model.charging.form = None;
        }
        Tab::Power => {
            model.power.loaded = false;
            model.power.form = None;
        }
        Tab::Display => {
            model.display.loaded = false;
            model.display.form = None;
            model.display.original_json = None;
        }
        Tab::Webcam => {
            model.webcam.form = None;
        }
        _ => {}
    }
}

fn save_form(model: &mut Model) -> Option<Command> {
    match model.active_tab {
        Tab::Settings => {
            if let Some(ref form) = model.settings.form
                && form.is_dirty()
            {
                let json = settings_form_to_json(form);
                model.dirty = true;
                return Some(Command::SaveSettings(json));
            }
            None
        }
        Tab::Keyboard => {
            if let Some(ref form) = model.keyboard.form
                && form.is_dirty()
            {
                let json = keyboard_form_to_json(form);
                model.dirty = true;
                return Some(Command::SaveKeyboard(json));
            }
            None
        }
        Tab::Charging => {
            if let Some(ref form) = model.charging.form
                && form.is_dirty()
            {
                let start = form.field_by_label("Start (%)").and_then(|f| f.value.as_number()).unwrap_or(80.0);
                let end = form.field_by_label("End (%)").and_then(|f| f.value.as_number()).unwrap_or(100.0);
                if start >= end {
                    model.push_notification(crate::model::Notification {
                        message: "Start threshold must be less than end threshold".into(),
                        is_error: true,
                    });
                    model.dirty = true;
                    return None;
                }
                let json = charging_form_to_json(form);
                model.dirty = true;
                return Some(Command::SaveCharging(json));
            }
            None
        }
        Tab::Power => {
            if let Some(ref form) = model.power.form
                && form.is_dirty()
            {
                let json = power_form_to_json(form);
                model.dirty = true;
                return Some(Command::SavePowerSettings(json));
            }
            None
        }
        Tab::Display => {
            if let Some(ref form) = model.display.form
                && form.is_dirty()
                && let Some(ref original) = model.display.original_json
            {
                let json = display_form_to_json(form, original);
                model.dirty = true;
                return Some(Command::SaveDisplay(json));
            }
            None
        }
        Tab::Webcam => {
            if let Some(ref form) = model.webcam.form
                && form.is_dirty()
            {
                let device = model
                    .webcam
                    .devices
                    .get(model.webcam.selected_device)
                    .map(|d| d.path.clone());
                match device {
                    Some(path) if !path.is_empty() => {
                        let json = webcam_form_to_json(form);
                        model.dirty = true;
                        return Some(Command::SaveWebcamControls { device: path, json });
                    }
                    _ => {
                        model.push_notification(crate::model::Notification {
                            message: "No webcam device selected".into(),
                            is_error: true,
                        });
                        model.dirty = true;
                        return None;
                    }
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> Msg {
        Msg::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn ctrl_key(code: KeyCode) -> Msg {
        Msg::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn quit_on_q() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('q')));
        assert!(model.should_quit);
        assert_eq!(cmds, vec![Command::Quit]);
    }

    #[test]
    fn quit_on_ctrl_c() {
        let mut model = Model::default();
        let cmds = update(&mut model, ctrl_key(KeyCode::Char('c')));
        assert!(model.should_quit);
        assert_eq!(cmds, vec![Command::Quit]);
    }

    #[test]
    fn fan_data_updates_dashboard() {
        let mut model = Model::default();
        let fan = crate::msg::FanState {
            speeds_percent: vec![42],
        };
        let cmds = update(&mut model, Msg::Data(DataUpdate::FanData(fan)));
        assert!(cmds.is_empty());
        assert!(model.dirty);
        assert_eq!(model.dashboard.fan.speeds_percent, vec![42]);
        assert_eq!(model.dashboard.fan_speed_history.len(), 1);
    }

    #[test]
    fn connection_lost_sets_status() {
        let mut model = Model::default();
        model.connection_status = crate::model::ConnectionStatus::Connected;
        update(&mut model, Msg::Data(DataUpdate::ConnectionLost));
        assert_eq!(
            model.connection_status,
            crate::model::ConnectionStatus::Disconnected
        );
        assert!(!model.notifications.is_empty());
    }

    #[test]
    fn resize_sets_dirty() {
        let mut model = Model {
            dirty: false,
            ..Default::default()
        };
        update(&mut model, Msg::Resize(120, 40));
        assert!(model.dirty);
    }

    #[test]
    fn switch_to_profiles_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('2')));
        assert_eq!(model.active_tab, Tab::Profiles);
        assert!(cmds.contains(&Command::FetchProfiles));
        assert!(cmds.contains(&Command::FetchAssignments));
    }

    #[test]
    fn profile_list_navigation() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.profiles = vec![
            crate::msg::ProfileSummary {
                id: "a".into(),
                name: "A".into(),
                description: "".into(),
            },
            crate::msg::ProfileSummary {
                id: "b".into(),
                name: "B".into(),
                description: "".into(),
            },
        ];
        model.profiles.selected_index = 0;
        update(&mut model, key(KeyCode::Down));
        assert_eq!(model.profiles.selected_index, 1);
        update(&mut model, key(KeyCode::Down));
        assert_eq!(model.profiles.selected_index, 1); // clamped
        update(&mut model, key(KeyCode::Up));
        assert_eq!(model.profiles.selected_index, 0);
    }

    #[test]
    fn profile_list_data_update() {
        let mut model = Model::default();
        model.profiles.selected_index = 5; // out of range
        let profiles = vec![crate::msg::ProfileSummary {
            id: "x".into(),
            name: "X".into(),
            description: "".into(),
        }];
        update(
            &mut model,
            Msg::Data(DataUpdate::ProfileList(profiles)),
        );
        assert_eq!(model.profiles.profiles.len(), 1);
        assert_eq!(model.profiles.selected_index, 0); // clamped
    }

    #[test]
    fn enter_opens_editor() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.profiles = vec![crate::msg::ProfileSummary {
            id: "test_id".into(),
            name: "Test".into(),
            description: "".into(),
        }];
        let cmds = update(&mut model, key(KeyCode::Enter));
        assert_eq!(
            model.profiles.view,
            ProfileView::Editor {
                profile_id: "test_id".into()
            }
        );
        assert!(cmds.contains(&Command::FetchProfileDetail("test_id".into())));
    }

    #[test]
    fn esc_returns_to_list() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.view = ProfileView::Editor {
            profile_id: "x".into(),
        };
        update(&mut model, key(KeyCode::Esc));
        assert_eq!(model.profiles.view, ProfileView::List);
    }

    #[test]
    fn cannot_delete_default_profile() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.profiles = vec![crate::msg::ProfileSummary {
            id: "__office__".into(),
            name: "Office".into(),
            description: "".into(),
        }];
        let cmds = update(&mut model, key(KeyCode::Char('d')));
        assert!(cmds.is_empty()); // no delete command
        assert!(!model.notifications.is_empty()); // error shown
    }

    #[test]
    fn switch_to_fan_curve_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('3')));
        assert_eq!(model.active_tab, Tab::FanCurve);
        assert!(cmds.contains(&Command::FetchActiveFanCurve));
    }

    #[test]
    fn fan_curve_navigation() {
        let mut model = Model {
            active_tab: Tab::FanCurve,
            ..Default::default()
        };
        model.fan_curve.curve_points = vec![(20.0, 10.0), (50.0, 40.0), (80.0, 70.0)];
        model.fan_curve.selected_point = 0;

        update(&mut model, key(KeyCode::Right));
        assert_eq!(model.fan_curve.selected_point, 1);
        update(&mut model, key(KeyCode::Right));
        assert_eq!(model.fan_curve.selected_point, 2);
        update(&mut model, key(KeyCode::Right));
        assert_eq!(model.fan_curve.selected_point, 2); // clamped
        update(&mut model, key(KeyCode::Left));
        assert_eq!(model.fan_curve.selected_point, 1);
    }

    #[test]
    fn power_state_update() {
        let mut model = Model::default();
        assert!(model.dashboard.power_on_ac.is_none());
        update(
            &mut model,
            Msg::Data(DataUpdate::PowerState { on_ac: true }),
        );
        assert_eq!(model.dashboard.power_on_ac, Some(true));
    }

    #[test]
    fn cpu_metrics_update() {
        let mut model = Model::default();
        let cpu = crate::msg::CpuState {
            temperature: Some(72.5),
            avg_frequency_mhz: Some(3200.0),
            core_count: Some(8),
        };
        update(&mut model, Msg::Data(DataUpdate::CpuMetrics(cpu)));
        assert_eq!(model.dashboard.cpu.temperature, Some(72.5));
        assert_eq!(model.dashboard.cpu.avg_frequency_mhz, Some(3200.0));
        assert_eq!(model.dashboard.cpu.core_count, Some(8));
        assert_eq!(model.dashboard.cpu_temp_history.len(), 1);
    }

    #[test]
    fn fan_curve_data_update() {
        let mut model = Model::default();
        model.fan_curve.selected_point = 5; // out of range
        update(
            &mut model,
            Msg::Data(DataUpdate::FanCurveData {
                profile_name: "Office".into(),
                fan_profile: "Quiet".into(),
                curve_cpu: vec![(20.0, 10.0), (50.0, 40.0)],
            }),
        );
        assert_eq!(model.fan_curve.fan_profile_name, "Quiet");
        assert_eq!(model.fan_curve.curve_points.len(), 2);
        assert_eq!(model.fan_curve.selected_point, 1); // clamped
        assert_eq!(
            model.dashboard.active_profile_name.as_deref(),
            Some("Office")
        );
    }

    // ─── Phase 4 tests ─────────────────────────────────────────────

    #[test]
    fn switch_to_settings_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('4')));
        assert_eq!(model.active_tab, Tab::Settings);
        assert!(cmds.contains(&Command::FetchSettings));
    }

    #[test]
    fn switch_to_keyboard_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('5')));
        assert_eq!(model.active_tab, Tab::Keyboard);
        assert!(cmds.contains(&Command::FetchKeyboard));
    }

    #[test]
    fn switch_to_charging_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('6')));
        assert_eq!(model.active_tab, Tab::Charging);
        assert!(cmds.contains(&Command::FetchCharging));
    }

    #[test]
    fn settings_data_populates_form() {
        let mut model = Model::default();
        let json = r#"{"fahrenheit":true,"cpuSettingsEnabled":false,"fanControlEnabled":true}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::SettingsData(json.into())),
        );
        assert!(model.settings.loaded);
        let form = model.settings.form.as_ref().unwrap();
        // Field 1 = Use Fahrenheit (toggle)
        assert_eq!(form.fields[1].value.as_bool(), Some(true));
        // Field 3 = CPU Settings (toggle)
        assert_eq!(form.fields[3].value.as_bool(), Some(false));
    }

    #[test]
    fn keyboard_data_populates_form() {
        let mut model = Model::default();
        let json = r##"{"brightness":75.0,"color":"#ff0000","mode":"Wave"}"##;
        update(
            &mut model,
            Msg::Data(DataUpdate::KeyboardData(json.into())),
        );
        assert!(model.keyboard.loaded);
        let form = model.keyboard.form.as_ref().unwrap();
        assert_eq!(form.fields[1].value.as_number(), Some(75.0));
        assert_eq!(form.fields[2].value.as_text(), Some("#ff0000"));
        assert_eq!(form.fields[3].value.as_select(), Some(2)); // Wave
    }

    #[test]
    fn charging_data_populates_form() {
        let mut model = Model::default();
        model.capabilities.charge_thresholds = true;
        model.capabilities.charging_profile = true;
        let json = r#"{"chargingProfile":"Reduced","chargingPriority":"Performance","startThreshold":60.0,"endThreshold":90.0}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::ChargingData(json.into())),
        );
        assert!(model.charging.loaded);
        let form = model.charging.form.as_ref().unwrap();
        assert_eq!(form.fields[1].value.as_select(), Some(1)); // Reduced
        assert_eq!(form.fields[2].value.as_select(), Some(1)); // Performance
        assert_eq!(form.fields[4].value.as_number(), Some(60.0));
        assert_eq!(form.fields[5].value.as_number(), Some(90.0));
    }

    #[test]
    fn settings_esc_discards() {
        let mut model = Model {
            active_tab: Tab::Settings,
            ..Default::default()
        };
        let json = r#"{"fahrenheit":false,"cpuSettingsEnabled":true,"fanControlEnabled":true}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::SettingsData(json.into())),
        );
        assert!(model.settings.form.is_some());

        update(&mut model, key(KeyCode::Esc));
        assert!(model.settings.form.is_none());
        assert!(!model.settings.loaded);
    }

    #[test]
    fn settings_save_emits_command() {
        let mut model = Model {
            active_tab: Tab::Settings,
            ..Default::default()
        };
        let json = r#"{"fahrenheit":false,"cpuSettingsEnabled":true,"fanControlEnabled":true}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::SettingsData(json.into())),
        );
        // Modify a field to make form dirty
        if let Some(ref mut form) = model.settings.form {
            form.focused = 1; // Use Fahrenheit toggle
            form.handle_key(KeyCode::Char(' '));
        }
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveSettings(_))));
    }

    #[test]
    fn profile_editor_form_created_on_detail() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.view = ProfileView::Editor {
            profile_id: "custom_1".into(),
        };
        let json = r#"{"id":"custom_1","name":"Test","description":"A test","cpu":{"governor":"performance","energyPerformancePreference":"performance","noTurbo":false},"display":{"brightness":80,"useBrightness":true},"fan":{"fanProfile":"Balanced","useControl":true,"minimumFanspeed":10,"maximumFanspeed":90}}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::ProfileDetail(json.into())),
        );
        let form = model.profiles.editor_form.as_ref().unwrap();
        // Name should be editable text (not read-only since non-default)
        assert!(form.fields[0].is_editable());
        assert_eq!(form.fields[0].value.as_text(), Some("Test"));
    }

    #[test]
    fn default_profile_editor_is_read_only() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.view = ProfileView::Editor {
            profile_id: "__office__".into(),
        };
        let json = r#"{"id":"__office__","name":"Office","description":"Standard","cpu":{"governor":"schedutil","energyPerformancePreference":"balance_performance","noTurbo":false},"display":{"brightness":60,"useBrightness":true},"fan":{"fanProfile":"Quiet","useControl":true,"minimumFanspeed":0,"maximumFanspeed":100}}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::ProfileDetail(json.into())),
        );
        let form = model.profiles.editor_form.as_ref().unwrap();
        // All fields should be read-only for default profiles
        assert!(!form.fields[0].is_editable()); // Name
        assert!(!form.fields[1].is_editable()); // Description
    }

    #[test]
    fn profile_editor_save_emits_command() {
        let mut model = Model {
            active_tab: Tab::Profiles,
            ..Default::default()
        };
        model.profiles.view = ProfileView::Editor {
            profile_id: "custom_1".into(),
        };
        let json = r#"{"id":"custom_1","name":"Test","description":"A test","cpu":{"governor":"performance","energyPerformancePreference":"performance","noTurbo":false},"display":{"brightness":80,"useBrightness":true},"fan":{"fanProfile":"Balanced","useControl":true,"minimumFanspeed":10,"maximumFanspeed":90}}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::ProfileDetail(json.into())),
        );

        // Edit the name
        if let Some(ref mut form) = model.profiles.editor_form {
            form.focused = 0;
            form.handle_key(KeyCode::Char('!'));
        }
        model.profiles.editor_dirty = true;

        // Move focus off the text field so 's' triggers save, not text input
        if let Some(ref mut form) = model.profiles.editor_form {
            form.handle_key(KeyCode::Down); // Name → Description (text)
            form.handle_key(KeyCode::Down); // Description → Governor (select)
        }

        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveProfile { .. })));
        if let Some(Command::SaveProfile { json, .. }) = cmds.iter().find(|c| matches!(c, Command::SaveProfile { .. })) {
            assert!(json.contains("Test!"));
        }
    }

    #[test]
    fn no_duplicate_settings_fetch() {
        let mut model = Model::default();
        // Load settings first time
        update(&mut model, key(KeyCode::Char('4')));
        assert!(model.settings.loaded == false); // not loaded yet, just requested

        // Simulate receiving data
        let json = r#"{"fahrenheit":false,"cpuSettingsEnabled":true,"fanControlEnabled":true}"#;
        update(
            &mut model,
            Msg::Data(DataUpdate::SettingsData(json.into())),
        );
        assert!(model.settings.loaded);

        // Switch away and back
        update(&mut model, key(KeyCode::Char('1')));
        let cmds = update(&mut model, key(KeyCode::Char('4')));
        // Should NOT fetch again since already loaded
        assert!(!cmds.contains(&Command::FetchSettings));
    }

    // ─── Form round-trip tests ──────────────────────────────────────

    #[test]
    fn settings_form_roundtrip() {
        let json = r#"{"fahrenheit":true,"cpuSettingsEnabled":false,"fanControlEnabled":true}"#;
        let form = parse_settings_form(json);
        let result = settings_form_to_json(&form);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["fahrenheit"], true);
        assert_eq!(v["cpuSettingsEnabled"], false);
        assert_eq!(v["fanControlEnabled"], true);
    }

    #[test]
    fn keyboard_form_roundtrip() {
        let json = r##"{"brightness":75.0,"color":"#ff00aa","mode":"Wave"}"##;
        let form = parse_keyboard_form(json);
        let result = keyboard_form_to_json(&form);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["brightness"], 75.0);
        assert_eq!(v["color"], "#ff00aa");
        assert_eq!(v["mode"], "Wave");
    }

    #[test]
    fn charging_form_roundtrip() {
        let json = r#"{"chargingProfile":"Stationary","chargingPriority":"Performance","startThreshold":40.0,"endThreshold":80.0}"#;
        let caps = crate::model::Capabilities { charge_thresholds: true, charging_profile: true, ..Default::default() };
        let form = parse_charging_form(json, &caps);
        let result = charging_form_to_json(&form);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["chargingProfile"], "Stationary");
        assert_eq!(v["chargingPriority"], "Performance");
        assert_eq!(v["startThreshold"], 40.0);
        assert_eq!(v["endThreshold"], 80.0);
    }

    #[test]
    fn profile_form_roundtrip() {
        let json = r#"{"id":"c1","name":"Gaming","description":"High perf","cpu":{"governor":"performance","energyPerformancePreference":"performance","noTurbo":false,"onlineCores":8},"display":{"brightness":100,"useBrightness":true},"fan":{"fanProfile":"Overboost","useControl":true,"minimumFanspeed":20,"maximumFanspeed":100}}"#;
        let form = parse_profile_form(json, false);
        let result = apply_profile_form(&form, json);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["name"], "Gaming");
        assert_eq!(v["cpu"]["governor"], "performance");
        assert_eq!(v["cpu"]["energyPerformancePreference"], "performance");
        assert_eq!(v["cpu"]["noTurbo"], false);
        assert_eq!(v["display"]["brightness"], 100);
        assert_eq!(v["display"]["useBrightness"], true);
        assert_eq!(v["fan"]["fanProfile"], "Overboost");
        assert_eq!(v["fan"]["minimumFanspeed"], 20);
        assert_eq!(v["fan"]["maximumFanspeed"], 100);
    }

    #[test]
    fn profile_form_edit_roundtrip() {
        let json = r#"{"id":"c1","name":"Test","description":"","cpu":{"governor":"powersave","energyPerformancePreference":"power","noTurbo":false},"display":{"brightness":50,"useBrightness":false},"fan":{"fanProfile":"Balanced","useControl":true,"minimumFanspeed":0,"maximumFanspeed":100}}"#;
        let mut form = parse_profile_form(json, false);
        // Change governor to "performance" (index 1)
        let gov_field = form.fields.iter_mut().find(|f| f.label == "Governor").unwrap();
        gov_field.value = crate::widgets::form::FieldValue::Select(1);
        let result = apply_profile_form(&form, json);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["cpu"]["governor"], "performance");
        // Other fields unchanged
        assert_eq!(v["name"], "Test");
        assert_eq!(v["fan"]["fanProfile"], "Balanced");
    }

    #[test]
    fn charging_threshold_validation_rejects_inverted() {
        let mut model = Model {
            active_tab: Tab::Charging,
            ..Default::default()
        };
        model.capabilities.charge_thresholds = true;
        let json = r#"{"chargingProfile":"Full Capacity","chargingPriority":"Battery","startThreshold":90.0,"endThreshold":40.0}"#;
        update(&mut model, Msg::Data(DataUpdate::ChargingData(json.into())));
        // Form is dirty because start > end is loaded from daemon
        if let Some(ref mut form) = model.charging.form {
            form.fields.iter_mut().for_each(|f| f.dirty = true);
        }
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        // Should NOT emit save command due to validation
        assert!(!cmds.iter().any(|c| matches!(c, Command::SaveCharging(_))));
        // Should have error notification
        assert!(model.notifications.iter().any(|n| n.is_error));
    }

    // ─── Phase 5 tests ─────────────────────────────────────────────

    #[test]
    fn switch_to_power_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('7')));
        assert_eq!(model.active_tab, Tab::Power);
        assert!(cmds.contains(&Command::FetchGpuInfo));
        assert!(cmds.contains(&Command::FetchPowerSettings));
    }

    #[test]
    fn switch_to_display_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('8')));
        assert_eq!(model.active_tab, Tab::Display);
        assert!(cmds.contains(&Command::FetchDisplay));
    }

    #[test]
    fn switch_to_webcam_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('9')));
        assert_eq!(model.active_tab, Tab::Webcam);
        assert!(cmds.contains(&Command::FetchWebcamDevices));
    }

    #[test]
    fn no_duplicate_power_fetch() {
        let mut model = Model::default();
        update(&mut model, key(KeyCode::Char('7')));
        // Simulate receiving data
        let gpu_json = r#"{"dgpuName":"RTX 3060","dgpuTemp":45.0,"dgpuUsage":5.0,"dgpuPowerDraw":15.0,"igpuName":"Intel UHD","igpuUsage":10.0,"primeMode":"on-demand","tgpOffset":0.0}"#;
        let power_json = r#"{"primeMode":"on-demand","tgpOffset":0.0,"shutdownHours":0,"shutdownMinutes":0,"shutdownActive":false}"#;
        update(&mut model, Msg::Data(DataUpdate::GpuData(gpu_json.into())));
        update(&mut model, Msg::Data(DataUpdate::PowerData(power_json.into())));
        assert!(model.power.loaded);
        // Switch away and back
        update(&mut model, key(KeyCode::Char('1')));
        let cmds = update(&mut model, key(KeyCode::Char('7')));
        assert!(!cmds.contains(&Command::FetchGpuInfo));
        assert!(!cmds.contains(&Command::FetchPowerSettings));
    }

    #[test]
    fn gpu_data_populates_model() {
        let mut model = Model::default();
        let json = r#"{"dgpuName":"RTX 4080","dgpuTemp":55.0,"dgpuUsage":30.0,"dgpuPowerDraw":120.0,"igpuName":"Intel Iris","igpuUsage":5.0,"primeMode":"performance","tgpOffset":10.0}"#;
        update(&mut model, Msg::Data(DataUpdate::GpuData(json.into())));
        let gpu = model.power.gpu_info.as_ref().unwrap();
        assert_eq!(gpu.dgpu_name, "RTX 4080");
        assert_eq!(gpu.dgpu_temp, Some(55.0));
        assert_eq!(gpu.dgpu_usage, Some(30.0));
        assert_eq!(gpu.dgpu_power_draw, Some(120.0));
        assert_eq!(gpu.igpu_name, "Intel Iris");
        assert_eq!(gpu.igpu_usage, Some(5.0));
    }

    #[test]
    fn power_data_populates_form() {
        let mut model = Model::default();
        let json = r#"{"primeMode":"performance","tgpOffset":15.0,"shutdownHours":2,"shutdownMinutes":30,"shutdownActive":true}"#;
        update(&mut model, Msg::Data(DataUpdate::PowerData(json.into())));
        assert!(model.power.loaded);
        let form = model.power.form.as_ref().unwrap();
        // PRIME Mode select: "performance" is index 1
        assert_eq!(form.field_by_label("PRIME Mode").unwrap().value.as_select(), Some(1));
        assert_eq!(form.field_by_label("TGP Offset (W)").unwrap().value.as_number(), Some(15.0));
        assert_eq!(form.field_by_label("Shutdown Active").unwrap().value.as_bool(), Some(true));
        assert_eq!(form.field_by_label("Hours").unwrap().value.as_number(), Some(2.0));
        assert_eq!(form.field_by_label("Minutes").unwrap().value.as_number(), Some(30.0));
    }

    #[test]
    fn display_data_populates_form() {
        let mut model = Model::default();
        let json = r#"{"brightness":90.0,"refreshRates":[60,120,144],"selectedRefreshRate":144,"resolutions":["1920x1080","2560x1440"],"selectedResolution":"2560x1440","ycbcr":true}"#;
        update(&mut model, Msg::Data(DataUpdate::DisplayData(json.into())));
        assert!(model.display.loaded);
        let form = model.display.form.as_ref().unwrap();
        assert_eq!(form.field_by_label("Brightness").unwrap().value.as_number(), Some(90.0));
        assert_eq!(form.field_by_label("Refresh Rate").unwrap().value.as_select(), Some(2)); // 144 Hz
        assert_eq!(form.field_by_label("Resolution").unwrap().value.as_select(), Some(1)); // 2560x1440
        assert_eq!(form.field_by_label("YCbCr 4:2:0").unwrap().value.as_bool(), Some(true));
    }

    #[test]
    fn webcam_devices_populates_model() {
        let mut model = Model::default();
        let json = r#"[{"path":"/dev/video0","name":"Integrated Webcam"},{"path":"/dev/video2","name":"USB Camera"}]"#;
        let cmds = update(&mut model, Msg::Data(DataUpdate::WebcamDevices(json.into())));
        assert_eq!(model.webcam.devices.len(), 2);
        assert_eq!(model.webcam.devices[0].name, "Integrated Webcam");
        assert_eq!(model.webcam.devices[1].path, "/dev/video2");
        assert!(model.webcam.loaded);
        // Should auto-fetch controls for first device
        assert!(cmds.contains(&Command::FetchWebcamControls("/dev/video0".into())));
    }

    #[test]
    fn webcam_controls_populates_form() {
        let mut model = Model::default();
        let json = r#"{"brightness":200.0,"contrast":100.0,"saturation":150.0,"sharpness":64.0,"autoExposure":false,"exposure":250.0,"autoWhiteBalance":true,"whiteBalance":5000.0}"#;
        update(&mut model, Msg::Data(DataUpdate::WebcamControls(json.into())));
        let form = model.webcam.form.as_ref().unwrap();
        assert_eq!(form.field_by_label("Brightness").unwrap().value.as_number(), Some(200.0));
        assert_eq!(form.field_by_label("Auto Exposure").unwrap().value.as_bool(), Some(false));
        assert_eq!(form.field_by_label("White Balance").unwrap().value.as_number(), Some(5000.0));
    }

    #[test]
    fn power_form_roundtrip() {
        let json = r#"{"primeMode":"offload","tgpOffset":-10.0,"shutdownHours":1,"shutdownMinutes":15,"shutdownActive":true}"#;
        let form = parse_power_form(json);
        let result = power_form_to_json(&form);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["primeMode"], "offload");
        assert_eq!(v["tgpOffset"], -10.0);
        assert_eq!(v["shutdownHours"], 1);
        assert_eq!(v["shutdownMinutes"], 15);
        assert_eq!(v["shutdownActive"], true);
    }

    #[test]
    fn display_form_roundtrip() {
        let json = r#"{"brightness":70.0,"refreshRates":[60,120,165],"selectedRefreshRate":120,"resolutions":["1920x1080","3840x2160"],"selectedResolution":"3840x2160","ycbcr":false}"#;
        let form = parse_display_form(json);
        let result = display_form_to_json(&form, json);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["brightness"], 70.0);
        assert_eq!(v["selectedRefreshRate"], 120);
        assert_eq!(v["selectedResolution"], "3840x2160");
        assert_eq!(v["ycbcr"], false);
        // Verify lists are preserved
        assert_eq!(v["refreshRates"].as_array().unwrap().len(), 3);
        assert_eq!(v["resolutions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn webcam_form_roundtrip() {
        let json = r#"{"brightness":180.0,"contrast":100.0,"saturation":200.0,"sharpness":50.0,"autoExposure":false,"exposure":300.0,"autoWhiteBalance":false,"whiteBalance":6500.0}"#;
        let form = parse_webcam_form(json);
        let result = webcam_form_to_json(&form);
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["brightness"], 180.0);
        assert_eq!(v["contrast"], 100.0);
        assert_eq!(v["saturation"], 200.0);
        assert_eq!(v["sharpness"], 50.0);
        assert_eq!(v["autoExposure"], false);
        assert_eq!(v["exposure"], 300.0);
        assert_eq!(v["autoWhiteBalance"], false);
        assert_eq!(v["whiteBalance"], 6500.0);
    }

    #[test]
    fn power_esc_discards() {
        let mut model = Model {
            active_tab: Tab::Power,
            ..Default::default()
        };
        let json = r#"{"primeMode":"on-demand","tgpOffset":0.0,"shutdownHours":0,"shutdownMinutes":0,"shutdownActive":false}"#;
        update(&mut model, Msg::Data(DataUpdate::PowerData(json.into())));
        assert!(model.power.form.is_some());
        update(&mut model, key(KeyCode::Esc));
        assert!(model.power.form.is_none());
        assert!(!model.power.loaded);
    }

    #[test]
    fn display_esc_discards() {
        let mut model = Model {
            active_tab: Tab::Display,
            ..Default::default()
        };
        let json = r#"{"brightness":80.0,"refreshRates":[60],"selectedRefreshRate":60,"resolutions":["1920x1080"],"selectedResolution":"1920x1080","ycbcr":false}"#;
        update(&mut model, Msg::Data(DataUpdate::DisplayData(json.into())));
        assert!(model.display.form.is_some());
        update(&mut model, key(KeyCode::Esc));
        assert!(model.display.form.is_none());
        assert!(!model.display.loaded);
    }

    #[test]
    fn power_save_emits_command() {
        let mut model = Model {
            active_tab: Tab::Power,
            ..Default::default()
        };
        let json = r#"{"primeMode":"on-demand","tgpOffset":0.0,"shutdownHours":0,"shutdownMinutes":0,"shutdownActive":false}"#;
        update(&mut model, Msg::Data(DataUpdate::PowerData(json.into())));
        // Modify a field
        if let Some(ref mut form) = model.power.form {
            form.focused = 1; // PRIME Mode select
            form.handle_key(KeyCode::Right); // cycle to "performance"
        }
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SavePowerSettings(_))));
    }

    #[test]
    fn webcam_save_emits_command_with_device() {
        let mut model = Model {
            active_tab: Tab::Webcam,
            ..Default::default()
        };
        model.webcam.devices = vec![crate::model::WebcamDevice {
            path: "/dev/video0".into(),
            name: "Webcam".into(),
        }];
        model.webcam.selected_device = 0;
        let json = r#"{"brightness":128.0,"contrast":128.0,"saturation":128.0,"sharpness":128.0,"autoExposure":true,"exposure":500.0,"autoWhiteBalance":true,"whiteBalance":4500.0}"#;
        update(&mut model, Msg::Data(DataUpdate::WebcamControls(json.into())));
        // Modify a field
        if let Some(ref mut form) = model.webcam.form {
            form.focused = 1; // Brightness number
            form.handle_key(KeyCode::Right); // increment
        }
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveWebcamControls { device, .. } if device == "/dev/video0")));
    }

    // ─── Phase 6 tests ─────────────────────────────────────────────

    #[test]
    fn switch_to_info_tab() {
        let mut model = Model::default();
        let cmds = update(&mut model, key(KeyCode::Char('0')));
        assert_eq!(model.active_tab, Tab::Info);
        assert!(cmds.contains(&Command::FetchSystemInfo));
    }

    #[test]
    fn no_duplicate_info_fetch() {
        let mut model = Model::default();
        update(&mut model, key(KeyCode::Char('0')));
        let json = r#"{"tccVersion":"1.0.0","daemonVersion":"1.0.0","hostname":"myhost","kernelVersion":"6.8.0"}"#;
        update(&mut model, Msg::Data(DataUpdate::SystemInfo(json.into())));
        assert!(model.info.loaded);
        update(&mut model, key(KeyCode::Char('1')));
        let cmds = update(&mut model, key(KeyCode::Char('0')));
        assert!(!cmds.contains(&Command::FetchSystemInfo));
    }

    #[test]
    fn system_info_populates_model() {
        let mut model = Model::default();
        let json = r#"{"tccVersion":"2.5.0","daemonVersion":"2.5.1","hostname":"tuxedo-box","kernelVersion":"6.10.3"}"#;
        update(&mut model, Msg::Data(DataUpdate::SystemInfo(json.into())));
        assert!(model.info.loaded);
        assert_eq!(model.info.tcc_version.as_deref(), Some("2.5.0"));
        assert_eq!(model.info.daemon_version.as_deref(), Some("2.5.1"));
        assert_eq!(model.info.hostname.as_deref(), Some("tuxedo-box"));
        assert_eq!(model.info.kernel_version.as_deref(), Some("6.10.3"));
    }

    #[test]
    fn help_overlay_toggle() {
        let mut model = Model::default();
        assert!(!model.help_visible);

        update(&mut model, key(KeyCode::Char('?')));
        assert!(model.help_visible);

        // While help is visible, other keys should not change tabs
        update(&mut model, key(KeyCode::Char('2')));
        assert_ne!(model.active_tab, Tab::Profiles);

        // ? closes help
        update(&mut model, key(KeyCode::Char('?')));
        assert!(!model.help_visible);
    }

    #[test]
    fn help_overlay_esc_closes() {
        let mut model = Model::default();
        update(&mut model, key(KeyCode::Char('?')));
        assert!(model.help_visible);
        update(&mut model, key(KeyCode::Esc));
        assert!(!model.help_visible);
    }

    #[test]
    fn small_terminal_warning() {
        let mut model = Model::default();
        update(&mut model, Msg::Resize(60, 20));
        assert!(model.notifications.iter().any(|n| n.is_error && n.message.contains("too small")));
    }

    #[test]
    fn adequate_terminal_no_warning() {
        let mut model = Model::default();
        update(&mut model, Msg::Resize(120, 40));
        assert!(!model.notifications.iter().any(|n| n.message.contains("too small")));
    }

    // ─── Fan curve editing tests ────────────────────────────────────

    fn fan_curve_model() -> Model {
        let mut model = Model {
            active_tab: Tab::FanCurve,
            ..Default::default()
        };
        model.fan_curve.curve_points = vec![(20.0, 10.0), (50.0, 40.0), (80.0, 70.0)];
        model.fan_curve.original_points = model.fan_curve.curve_points.clone();
        model.fan_curve.selected_point = 1;
        model
    }

    #[test]
    fn fan_curve_adjust_speed_up() {
        let mut model = fan_curve_model();
        update(&mut model, key(KeyCode::Up));
        assert_eq!(model.fan_curve.curve_points[1].1, 45.0);
        assert!(model.fan_curve.is_dirty());
    }

    #[test]
    fn fan_curve_adjust_speed_down() {
        let mut model = fan_curve_model();
        update(&mut model, key(KeyCode::Down));
        assert_eq!(model.fan_curve.curve_points[1].1, 35.0);
        assert!(model.fan_curve.is_dirty());
    }

    #[test]
    fn fan_curve_speed_clamped_at_100() {
        let mut model = fan_curve_model();
        model.fan_curve.curve_points[1].1 = 98.0;
        model.fan_curve.selected_point = 1;
        update(&mut model, key(KeyCode::Up));
        assert_eq!(model.fan_curve.curve_points[1].1, 100.0);
    }

    #[test]
    fn fan_curve_speed_clamped_at_0() {
        let mut model = fan_curve_model();
        model.fan_curve.curve_points[1].1 = 3.0;
        model.fan_curve.selected_point = 1;
        update(&mut model, key(KeyCode::Down));
        assert_eq!(model.fan_curve.curve_points[1].1, 0.0);
    }

    #[test]
    fn fan_curve_insert_point() {
        let mut model = fan_curve_model();
        // Insert after point 1 (50°C, 40%)
        update(&mut model, key(KeyCode::Char('i')));
        assert_eq!(model.fan_curve.curve_points.len(), 4);
        // New point is midpoint between (50,40) and (80,70)
        assert_eq!(model.fan_curve.curve_points[2], (65.0, 55.0));
        assert_eq!(model.fan_curve.selected_point, 2);
        assert!(model.fan_curve.is_dirty());
    }

    #[test]
    fn fan_curve_insert_at_end() {
        let mut model = fan_curve_model();
        model.fan_curve.selected_point = 2; // last point (80, 70)
        update(&mut model, key(KeyCode::Char('i')));
        assert_eq!(model.fan_curve.curve_points.len(), 4);
        // Midpoint between (80,70) and (100,100)
        assert_eq!(model.fan_curve.curve_points[3], (90.0, 85.0));
        assert_eq!(model.fan_curve.selected_point, 3);
    }

    #[test]
    fn fan_curve_delete_point() {
        let mut model = fan_curve_model();
        model.fan_curve.selected_point = 1;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 2);
        assert_eq!(model.fan_curve.curve_points[1], (80.0, 70.0));
        assert!(model.fan_curve.is_dirty());
    }

    #[test]
    fn fan_curve_delete_min_two_points() {
        let mut model = fan_curve_model();
        model.fan_curve.curve_points = vec![(20.0, 10.0), (50.0, 40.0), (80.0, 70.0)];
        model.fan_curve.original_points = model.fan_curve.curve_points.clone();
        // Can't delete first point
        model.fan_curve.selected_point = 0;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 3);
        // Can't delete last point
        model.fan_curve.selected_point = 2;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 3);
    }

    #[test]
    fn fan_curve_delete_protects_first_last() {
        let mut model = fan_curve_model();
        // Insert extra points so min-2 isn't the blocker
        model.fan_curve.curve_points = vec![
            (0.0, 0.0), (30.0, 20.0), (60.0, 50.0), (100.0, 100.0),
        ];
        model.fan_curve.original_points = model.fan_curve.curve_points.clone();
        // Try deleting first
        model.fan_curve.selected_point = 0;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 4);
        // Try deleting last
        model.fan_curve.selected_point = 3;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 4);
        // Middle point deletes fine
        model.fan_curve.selected_point = 1;
        update(&mut model, key(KeyCode::Char('x')));
        assert_eq!(model.fan_curve.curve_points.len(), 3);
    }

    #[test]
    fn fan_curve_reset_to_default() {
        let mut model = fan_curve_model();
        update(&mut model, key(KeyCode::Char('r')));
        assert_eq!(model.fan_curve.curve_points.len(), 5);
        assert_eq!(model.fan_curve.curve_points[0], (0.0, 0.0));
        assert_eq!(model.fan_curve.curve_points[4], (100.0, 100.0));
        assert_eq!(model.fan_curve.selected_point, 0);
        assert!(model.fan_curve.is_dirty());
    }

    #[test]
    fn fan_curve_poller_skips_when_dirty() {
        let mut model = fan_curve_model();
        // Make the curve dirty by adjusting speed
        update(&mut model, key(KeyCode::Up));
        assert!(model.fan_curve.is_dirty());
        let edited = model.fan_curve.curve_points.clone();
        // Simulate poller data arriving
        update(
            &mut model,
            Msg::Data(DataUpdate::FanCurveData {
                profile_name: "Office".into(),
                fan_profile: "Quiet".into(),
                curve_cpu: vec![(10.0, 5.0), (90.0, 95.0)],
            }),
        );
        // Edited points should be preserved
        assert_eq!(model.fan_curve.curve_points, edited);
        // But original_points should update
        assert_eq!(model.fan_curve.original_points, vec![(10.0, 5.0), (90.0, 95.0)]);
    }

    #[test]
    fn fan_curve_save_emits_command() {
        let mut model = fan_curve_model();
        // Modify a point
        update(&mut model, key(KeyCode::Up));
        assert!(model.fan_curve.is_dirty());
        // Save
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveFanCurve(_))));
        // Dirty flag cleared immediately
        assert!(!model.fan_curve.is_dirty());
        assert_eq!(model.fan_curve.original_points, model.fan_curve.curve_points);
    }

    #[test]
    fn fan_curve_save_no_op_when_clean() {
        let mut model = fan_curve_model();
        let cmds = update(&mut model, key(KeyCode::Char('s')));
        assert!(!cmds.iter().any(|c| matches!(c, Command::SaveFanCurve(_))));
    }

    #[test]
    fn fan_curve_discard_restores_original() {
        let mut model = fan_curve_model();
        update(&mut model, key(KeyCode::Up)); // modify
        assert!(model.fan_curve.is_dirty());
        update(&mut model, key(KeyCode::Esc)); // discard
        assert!(!model.fan_curve.is_dirty());
        assert_eq!(model.fan_curve.curve_points, model.fan_curve.original_points);
    }

    #[test]
    fn fan_curve_to_json_roundtrip() {
        let points = vec![(20.0, 10.0), (50.0, 40.0), (80.0, 70.0)];
        let json = super::fan_curve_to_json(&points);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["temp"], 20);
        assert_eq!(parsed[0]["speed"], 10);
        assert_eq!(parsed[2]["temp"], 80);
        assert_eq!(parsed[2]["speed"], 70);
    }
}
