use std::fs;
use std::time::Duration;

use sysinfo::System;

#[derive(Clone, Debug)]
pub struct SystemSnapshot {
    pub os_name: String,
    pub distro_id: String,
    pub host_name: String,
    pub kernel: String,
    pub cpu_name: String,
    pub logical_cores: usize,
    pub physical_cores: usize,
    pub cpu_usage: f32,
    pub used_memory: u64,
    pub total_memory: u64,
    pub used_swap: u64,
    pub total_swap: u64,
    pub uptime_seconds: u64,
}

pub struct SystemSampler {
    system: System,
}

impl SystemSampler {
    pub fn new() -> Result<Self, String> {
        // Do not enumerate every running process just to render a fetch screen.
        let mut system = System::new().map_err(|error| {
            format!("could not initialize system information: {error}")
        })?;
        system.refresh_cpu_all();
        system.refresh_memory();

        // CPU usage is calculated from deltas; the first sample is only a baseline.
        std::thread::sleep(Duration::from_millis(120));
        system.refresh_cpu_usage();

        Ok(Self { system })
    }

    pub fn refresh(&mut self) -> SystemSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.snapshot()
    }

    pub fn snapshot(&self) -> SystemSnapshot {
        let logical_cores = self.system.cpus().len();
        let physical_cores = System::physical_core_count()
            .ok()
            .flatten()
            .unwrap_or(logical_cores);

        let cpu_name = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        let (os_name, distro_id) = detect_distribution();
        let host_name = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let kernel = System::kernel_version().unwrap_or_else(|| "unknown".to_string());

        SystemSnapshot {
            os_name,
            distro_id,
            host_name,
            kernel,
            cpu_name,
            logical_cores,
            physical_cores,
            cpu_usage: self.system.global_cpu_usage().clamp(0.0, 100.0),
            used_memory: self.system.used_memory(),
            total_memory: self.system.total_memory(),
            used_swap: self.system.used_swap(),
            total_swap: self.system.total_swap(),
            uptime_seconds: System::uptime(),
        }
    }
}

fn detect_distribution() -> (String, String) {
    #[cfg(target_os = "linux")]
    {
        let pretty_name = os_release_value("PRETTY_NAME")
            .unwrap_or_else(|| "Linux".to_string());
        let distro_id = os_release_value("ID")
            .or_else(|| os_release_value("ID_LIKE"))
            .unwrap_or_else(|| "linux".to_string())
            .split_whitespace()
            .next()
            .unwrap_or("linux")
            .to_ascii_lowercase();
        return (pretty_name, distro_id);
    }

    #[cfg(target_os = "macos")]
    {
        let version = System::os_version().unwrap_or_default();
        return (
            if version.is_empty() { "macOS".to_string() } else { format!("macOS {version}") },
            "macos".to_string(),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let version = System::os_version().unwrap_or_default();
        return (
            if version.is_empty() { "Windows".to_string() } else { format!("Windows {version}") },
            "windows".to_string(),
        );
    }

    #[cfg(target_os = "freebsd")]
    {
        let version = System::os_version().unwrap_or_default();
        return (
            if version.is_empty() { "FreeBSD".to_string() } else { format!("FreeBSD {version}") },
            "freebsd".to_string(),
        );
    }

    #[cfg(target_os = "netbsd")]
    {
        let version = System::os_version().unwrap_or_default();
        return (
            if version.is_empty() { "NetBSD".to_string() } else { format!("NetBSD {version}") },
            "netbsd".to_string(),
        );
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_os = "freebsd",
        target_os = "netbsd"
    )))]
    {
        let name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
        let version = System::os_version().unwrap_or_default();
        let label = if version.is_empty() { name } else { format!("{name} {version}") };
        (label, std::env::consts::OS.to_string())
    }
}

#[cfg(target_os = "linux")]
fn os_release_value(key: &str) -> Option<String> {
    let contents = fs::read_to_string("/etc/os-release").ok()?;
    let prefix = format!("{key}=");
    contents.lines().find_map(|line| {
        let value = line.strip_prefix(&prefix)?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        (!value.is_empty()).then(|| value.to_string())
    })
}

#[cfg(not(target_os = "linux"))]
fn os_release_value(_key: &str) -> Option<String> {
    None
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds / 3_600) % 24;
    let minutes = (seconds / 60) % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_uptime};

    #[test]
    fn formats_memory_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
    }

    #[test]
    fn formats_uptime() {
        assert_eq!(format_uptime(61), "1m");
        assert_eq!(format_uptime(3_661), "1h 1m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }
}
