use crate::io::TuxedoIO;
use crate::profiles::FanTableEntry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

/// Fan control mode: either a flat manual speed or a temperature-based curve.
#[derive(Debug, Clone)]
pub enum FanMode {
    /// Fixed speed percentage (0-100), set via D-Bus set_fan_speed_percent.
    Manual(i32),
    /// Temperature→speed curve from the active profile's fan table.
    Curve(Vec<FanTableEntry>),
}

pub struct FanControlTask {
    io: Arc<dyn TuxedoIO + Send + Sync>,
    pub active: Arc<Mutex<bool>>,
    /// Per-fan mode: fan_idx → FanMode
    pub modes: Arc<Mutex<HashMap<i32, FanMode>>>,
    pub tick_duration_ms: u64,
}

/// Interpolate fan speed from a sorted fan curve table.
/// Below the first point → first point's speed.
/// Above the last point → last point's speed.
/// Between two points → linear interpolation.
pub fn interpolate_fan_curve(table: &[FanTableEntry], temp_c: f64) -> i32 {
    if table.is_empty() {
        return 0;
    }
    if table.len() == 1 || temp_c <= table[0].temp as f64 {
        return table[0].speed as i32;
    }
    let last = &table[table.len() - 1];
    if temp_c >= last.temp as f64 {
        return last.speed as i32;
    }
    // Find the two surrounding points
    for i in 1..table.len() {
        let lo = &table[i - 1];
        let hi = &table[i];
        if temp_c <= hi.temp as f64 {
            let t_range = (hi.temp as f64) - (lo.temp as f64);
            if t_range == 0.0 {
                return hi.speed as i32;
            }
            let fraction = (temp_c - lo.temp as f64) / t_range;
            let speed = lo.speed as f64 + fraction * (hi.speed as f64 - lo.speed as f64);
            return speed.round() as i32;
        }
    }
    last.speed as i32
}

impl FanControlTask {
    pub fn new(io: Arc<dyn TuxedoIO + Send + Sync>, tick_duration_ms: u64) -> Self {
        let initial_speed = io.get_fan_speed_percent(0).unwrap_or(0);
        let mut modes = HashMap::new();
        modes.insert(0, FanMode::Manual(initial_speed));
        Self {
            io,
            active: Arc::new(Mutex::new(true)),
            modes: Arc::new(Mutex::new(modes)),
            tick_duration_ms,
        }
    }

    /// Set a fixed manual fan speed for a specific fan (replaces any active curve).
    pub async fn set_manual_speed(&self, fan_idx: i32, speed: i32) {
        self.modes.lock().await.insert(fan_idx, FanMode::Manual(speed));
    }

    /// Load a fan curve for the CPU fan (index 0). The task will
    /// read CPU temperature each tick and interpolate the target speed.
    pub async fn set_cpu_curve(&self, table: Vec<FanTableEntry>) {
        self.modes.lock().await.insert(0, FanMode::Curve(table));
    }

    /// Load a fan curve for the GPU fan (index 1). Uses CPU temperature
    /// as a proxy if no GPU temp sensor is available.
    pub async fn set_gpu_curve(&self, table: Vec<FanTableEntry>) {
        self.modes.lock().await.insert(1, FanMode::Curve(table));
    }

    pub fn spawn(&self) -> tokio::task::JoinHandle<()> {
        let active = self.active.clone();
        let modes = self.modes.clone();
        let io = self.io.clone();
        let delay = Duration::from_millis(self.tick_duration_ms);

        tokio::spawn(async move {
            let fan_count = io.get_fan_count().unwrap_or(1);

            loop {
                let is_active = *active.lock().await;
                if is_active {
                    let temp = io.get_cpu_temperature().unwrap_or(50.0);
                    let current_modes = modes.lock().await;

                    for fan_idx in 0..fan_count as i32 {
                        let target = match current_modes.get(&fan_idx) {
                            Some(FanMode::Manual(speed)) => *speed,
                            Some(FanMode::Curve(table)) => interpolate_fan_curve(table, temp),
                            None => continue, // No mode set for this fan
                        };

                        let current = io.get_fan_speed_percent(fan_idx).unwrap_or(0);
                        if current != target {
                            let diff = target - current;
                            let step = diff.signum() * std::cmp::min(20, diff.abs());
                            let _ = io.set_fan_speed_percent(fan_idx, current + step);
                        }
                    }
                }

                sleep(delay).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::MockTuxedoIO;

    #[test]
    fn test_interpolation_empty() {
        assert_eq!(interpolate_fan_curve(&[], 50.0), 0);
    }

    #[test]
    fn test_interpolation_single_point() {
        let table = vec![FanTableEntry { temp: 50, speed: 40 }];
        assert_eq!(interpolate_fan_curve(&table, 30.0), 40);
        assert_eq!(interpolate_fan_curve(&table, 50.0), 40);
        assert_eq!(interpolate_fan_curve(&table, 70.0), 40);
    }

    #[test]
    fn test_interpolation_below_first() {
        let table = vec![
            FanTableEntry { temp: 40, speed: 20 },
            FanTableEntry { temp: 80, speed: 80 },
        ];
        assert_eq!(interpolate_fan_curve(&table, 10.0), 20);
    }

    #[test]
    fn test_interpolation_above_last() {
        let table = vec![
            FanTableEntry { temp: 40, speed: 20 },
            FanTableEntry { temp: 80, speed: 80 },
        ];
        assert_eq!(interpolate_fan_curve(&table, 95.0), 80);
    }

    #[test]
    fn test_interpolation_exact_points() {
        let table = vec![
            FanTableEntry { temp: 0, speed: 0 },
            FanTableEntry { temp: 40, speed: 20 },
            FanTableEntry { temp: 60, speed: 40 },
            FanTableEntry { temp: 80, speed: 70 },
            FanTableEntry { temp: 100, speed: 100 },
        ];
        assert_eq!(interpolate_fan_curve(&table, 0.0), 0);
        assert_eq!(interpolate_fan_curve(&table, 40.0), 20);
        assert_eq!(interpolate_fan_curve(&table, 60.0), 40);
        assert_eq!(interpolate_fan_curve(&table, 80.0), 70);
        assert_eq!(interpolate_fan_curve(&table, 100.0), 100);
    }

    #[test]
    fn test_interpolation_midpoints() {
        let table = vec![
            FanTableEntry { temp: 0, speed: 0 },
            FanTableEntry { temp: 100, speed: 100 },
        ];
        assert_eq!(interpolate_fan_curve(&table, 50.0), 50);
        assert_eq!(interpolate_fan_curve(&table, 25.0), 25);
        assert_eq!(interpolate_fan_curve(&table, 75.0), 75);
    }

    #[test]
    fn test_interpolation_non_linear() {
        let table = vec![
            FanTableEntry { temp: 40, speed: 20 },
            FanTableEntry { temp: 60, speed: 40 },
        ];
        // midpoint: temp=50 → speed=30
        assert_eq!(interpolate_fan_curve(&table, 50.0), 30);
        // quarter: temp=45 → speed=25
        assert_eq!(interpolate_fan_curve(&table, 45.0), 25);
    }

    #[tokio::test]
    async fn test_fan_curve_smoothing() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        let task = FanControlTask::new(mock_io.clone(), 5);
        let handle = task.spawn();

        task.set_manual_speed(0, 60).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let current = mock_io.get_fan_speed_percent(0).unwrap_or(0);
        assert_eq!(current, 60);

        *task.active.lock().await = false;
        handle.abort();
    }

    #[tokio::test]
    async fn test_fan_curve_mode() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        // Set CPU temp to 60°C
        *mock_io.cpu_temperature.write().unwrap() = 60.0;

        let task = FanControlTask::new(mock_io.clone(), 5);
        let handle = task.spawn();

        // Load a curve: 0°C→0%, 50°C→25%, 70°C→75%, 100°C→100%
        task.set_cpu_curve(vec![
            FanTableEntry { temp: 0, speed: 0 },
            FanTableEntry { temp: 50, speed: 25 },
            FanTableEntry { temp: 70, speed: 75 },
            FanTableEntry { temp: 100, speed: 100 },
        ])
        .await;

        // At 60°C, interpolation between (50,25) and (70,75): 25 + 0.5 * 50 = 50
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let current = mock_io.get_fan_speed_percent(0).unwrap_or(0);
        assert_eq!(current, 50);

        *task.active.lock().await = false;
        handle.abort();
    }

    #[tokio::test]
    async fn test_manual_overrides_curve() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        *mock_io.cpu_temperature.write().unwrap() = 60.0;

        let task = FanControlTask::new(mock_io.clone(), 5);
        let handle = task.spawn();

        // Start with curve
        task.set_cpu_curve(vec![
            FanTableEntry { temp: 0, speed: 0 },
            FanTableEntry { temp: 100, speed: 100 },
        ])
        .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let current = mock_io.get_fan_speed_percent(0).unwrap_or(0);
        assert_eq!(current, 60); // 60°C → 60%

        // Override with manual
        task.set_manual_speed(0, 30).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let current = mock_io.get_fan_speed_percent(0).unwrap_or(0);
        assert_eq!(current, 30);

        *task.active.lock().await = false;
        handle.abort();
    }

    #[tokio::test]
    async fn test_multi_fan_independent_control() {
        let mock_io = Arc::new(MockTuxedoIO::new());
        *mock_io.fan_count.write().unwrap() = 2;
        *mock_io.cpu_temperature.write().unwrap() = 50.0;

        let task = FanControlTask::new(mock_io.clone(), 5);
        let handle = task.spawn();

        // Fan 0: manual 40%, Fan 1: curve
        task.set_manual_speed(0, 40).await;
        task.set_gpu_curve(vec![
            FanTableEntry { temp: 0, speed: 0 },
            FanTableEntry { temp: 100, speed: 100 },
        ])
        .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let fan0 = mock_io.get_fan_speed_percent(0).unwrap_or(0);
        let fan1 = mock_io.get_fan_speed_percent(1).unwrap_or(0);
        assert_eq!(fan0, 40);
        assert_eq!(fan1, 50); // 50°C → 50% from curve

        *task.active.lock().await = false;
        handle.abort();
    }
}
