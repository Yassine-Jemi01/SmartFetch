use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Print, ResetColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::system::{format_bytes, format_uptime, SystemSampler, SystemSnapshot};

const SHADE_RAMP: &[u8] = b" .,:;irsXA253hMHGS#9B&@";
const FRAME_INTERVAL: Duration = Duration::from_millis(40);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
pub enum AnimationMode {
    DistroLogo,
    Donut,
}

struct Point {
    x: f32,
    y: f32,
    z: f32,
    nx: f32,
    ny: f32,
    nz: f32,
    shade: f32,
}

struct Scene {
    points: Vec<Point>,
    extent_x: f32,
    extent_y: f32,
}

struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<u8>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            cells: vec![b' '; len],
            depth: vec![f32::NEG_INFINITY; len],
        }
    }

    fn plot(&mut self, x: f32, y: f32, z: f32, brightness: f32) {
        let px = x.round() as isize;
        let py = y.round() as isize;
        if px < 0 || py < 0 || px >= self.width as isize || py >= self.height as isize {
            return;
        }

        let index = py as usize * self.width + px as usize;
        if z <= self.depth[index] {
            return;
        }

        self.depth[index] = z;
        let normalized = brightness.clamp(0.0, 1.0);
        let shade_index = (normalized * (SHADE_RAMP.len() - 1) as f32).round() as usize;
        self.cells[index] = SHADE_RAMP[shade_index];
    }

    fn lines(&self) -> Vec<String> {
        self.cells
            .chunks(self.width.max(1))
            .map(|row| String::from_utf8_lossy(row).trim_end().to_string())
            .collect()
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All)) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, ResetColor, Show, LeaveAlternateScreen);
        let _ = stdout.flush();
        let _ = terminal::disable_raw_mode();
    }
}

pub fn run_live(
    sampler: &mut SystemSampler,
    mode: AnimationMode,
    logo_name: &str,
) -> io::Result<()> {
    let _terminal = TerminalGuard::enter()?;
    let mut stdout = io::stdout();
    let mut snapshot = sampler.snapshot();
    let scene = Scene::new(mode, logo_name);
    let mut angle = 0.0_f32;
    let mut paused = false;
    let mut next_sample = Instant::now() + SAMPLE_INTERVAL;

    loop {
        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release && should_quit(key.code, key.modifiers) {
                    break;
                }
                if key.kind != KeyEventKind::Release {
                    match key.code {
                        KeyCode::Char(' ') => paused = !paused,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            snapshot = sampler.refresh();
                            next_sample = Instant::now() + SAMPLE_INTERVAL;
                        }
                        _ => {}
                    }
                }
            }
        }

        let now = Instant::now();
        if now >= next_sample {
            snapshot = sampler.refresh();
            next_sample = now + SAMPLE_INTERVAL;
        }

        if !paused {
            angle += 0.045;
        }

        let (columns, rows) = terminal::size()?;
        let frame = render_live_frame(
            &scene,
            angle,
            &snapshot,
            columns as usize,
            rows as usize,
            paused,
        );

        // One queued frame and one flush keeps terminal output smooth and cheap.
        queue!(
            stdout,
            crossterm::cursor::MoveTo(0, 0),
            Clear(ClearType::All),
            Print(frame)
        )?;
        stdout.flush()?;
        thread::sleep(FRAME_INTERVAL);
    }

    Ok(())
}

fn should_quit(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'))
        || (code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL))
}

fn render_live_frame(
    scene: &Scene,
    angle: f32,
    snapshot: &SystemSnapshot,
    columns: usize,
    rows: usize,
    paused: bool,
) -> String {
    let columns = columns.max(30);
    let rows = rows.max(10);
    let wide = columns >= 100 && rows >= 20;
    let (scene_width, scene_height) = if wide {
        ((columns / 2).min(50).saturating_sub(3).max(24), (rows - 3).min(22).max(10))
    } else {
        ((columns - 2).min(52).max(20), ((rows / 2).saturating_sub(2)).min(18).max(8))
    };

    let artwork = render_scene(scene, angle, scene_width, scene_height);
    let panel = info_lines(snapshot, columns.saturating_sub(scene_width + 4).max(20));
    let mut output = String::new();

    if wide {
        let height = artwork.len().max(panel.len());
        for row in 0..height {
            let art = artwork.get(row).map(String::as_str).unwrap_or("");
            let info = panel.get(row).map(String::as_str).unwrap_or("");
            output.push_str("\x1b[96m");
            output.push_str(&format!("{art:<scene_width$}"));
            output.push_str("\x1b[0m    ");
            output.push_str(info);
            output.push('\n');
        }
    } else {
        for line in artwork {
            output.push_str("\x1b[96m");
            output.push_str(&line);
            output.push_str("\x1b[0m\n");
        }
        output.push('\n');
        for line in panel {
            output.push_str(&line);
            output.push('\n');
        }
    }

    output.push('\n');
    output.push_str(if paused {
        "PAUSED  |  Q quit  Space resume  R refresh"
    } else {
        "LIVE  |  Q quit  Space pause  R refresh"
    });
    output
}

fn info_lines(snapshot: &SystemSnapshot, width: usize) -> Vec<String> {
    let cpu_bar = progress_bar(snapshot.cpu_usage, 14);
    let memory_percent = percentage(snapshot.used_memory, snapshot.total_memory);
    let memory_bar = progress_bar(memory_percent, 14);
    let swap_percent = percentage(snapshot.used_swap, snapshot.total_swap);
    let swap_bar = progress_bar(swap_percent, 14);

    vec![
        "SMARTFETCH  /  SYSTEM LIVE".to_string(),
        "────────────────────────────────".to_string(),
        format!("OS       {}", snapshot.os_name),
        format!("Host     {}", snapshot.host_name),
        format!("Kernel   {}", snapshot.kernel),
        format!("CPU      {}", snapshot.cpu_name),
        format!("Cores    {} logical / {} physical", snapshot.logical_cores, snapshot.physical_cores),
        format!("CPU      {cpu_bar} {:>5.1}%", snapshot.cpu_usage),
        format!("RAM      {memory_bar} {:>5.1}%", memory_percent),
        format!("         {} / {}", format_bytes(snapshot.used_memory), format_bytes(snapshot.total_memory)),
        format!("SWAP     {swap_bar} {:>5.1}%", swap_percent),
        format!("         {} / {}", format_bytes(snapshot.used_swap), format_bytes(snapshot.total_swap)),
        format!("Uptime   {}", format_uptime(snapshot.uptime_seconds)),
        " ".to_string(),
        "Refresh  1 second".to_string(),
    ]
    .into_iter()
    .map(|line| truncate(&line, width))
    .collect()
}

fn percentage(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 * 100.0 / total as f64).clamp(0.0, 100.0) as f32
    }
}

fn progress_bar(percent: f32, width: usize) -> String {
    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize;
    format!(
        "[{}{}]",
        "█".repeat(filled.min(width)),
        "░".repeat(width.saturating_sub(filled)),
    )
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    let mut shortened: String = value.chars().take(width - 3).collect();
    shortened.push_str("...");
    shortened
}

pub fn render_static(snapshot: &SystemSnapshot, logo_name: &str) -> String {
    let logo_lines = clean_logo(logo_for(logo_name));
    let columns = terminal::size().map(|(columns, _)| columns as usize).unwrap_or(80);
    let panel_width = columns.saturating_sub(2).min(64).max(20);
    let panel = info_lines(snapshot, panel_width);
    let logo_width = logo_lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let gap = 6;
    let mut output = String::new();

    // Preserve a deliberate gutter. Stack the blocks when the terminal is narrow.
    if columns >= logo_width + gap + panel_width {
        for row in 0..logo_lines.len().max(panel.len()) {
            let art = logo_lines.get(row).map(String::as_str).unwrap_or("");
            let info = panel.get(row).map(String::as_str).unwrap_or("");
            output.push_str(&format!("{art:<logo_width$}"));
            output.push_str(&" ".repeat(gap));
            output.push_str(info);
            output.push('\n');
        }
    } else {
        for line in &logo_lines {
            output.push_str(line);
            output.push('\n');
        }
        output.push('\n');
        for line in &panel {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

impl Scene {
    fn new(mode: AnimationMode, logo_name: &str) -> Self {
        match mode {
            AnimationMode::Donut => Self::donut(),
            AnimationMode::DistroLogo => Self::logo(logo_for(logo_name)),
        }
    }

    fn donut() -> Self {
        let mut points = Vec::with_capacity(3_000);
        let major = 1.0_f32;
        let minor = 0.43_f32;
        let mut theta = 0.0_f32;
        while theta < std::f32::consts::TAU {
            let (sin_theta, cos_theta) = theta.sin_cos();
            let mut phi = 0.0_f32;
            while phi < std::f32::consts::TAU {
                let (sin_phi, cos_phi) = phi.sin_cos();
                let ring = major + minor * cos_theta;
                points.push(Point {
                    x: ring * cos_phi,
                    y: ring * sin_phi,
                    z: minor * sin_theta,
                    nx: cos_theta * cos_phi,
                    ny: cos_theta * sin_phi,
                    nz: sin_theta,
                    shade: 1.0,
                });
                phi += 0.075;
            }
            theta += 0.18;
        }
        Self {
            points,
            extent_x: major + minor + 0.1,
            extent_y: major + minor + 0.1,
        }
    }

    fn logo(logo: &str) -> Self {
        let lines = clean_logo(logo);
        let height = lines.len().max(1);
        let width = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(1)
            .max(1);
        let mut points = Vec::new();

        for (row, line) in lines.iter().enumerate() {
            for (column, character) in line.chars().enumerate() {
                let density = char_density(character);
                if density < 0.12 {
                    continue;
                }

                let x = column as f32 - width as f32 / 2.0;
                let y = height as f32 / 2.0 - row as f32;
                let depth = 0.25 + density * 1.55;
                let slices = 5;
                for slice in 0..=slices {
                    let t = slice as f32 / slices as f32;
                    let z = -depth / 2.0 + depth * t;
                    let is_face = slice == 0 || slice == slices;
                    points.push(Point {
                        x,
                        y,
                        z,
                        nx: 0.0,
                        ny: 0.0,
                        nz: if slice == 0 { -1.0 } else if slice == slices { 1.0 } else { 0.22 },
                        shade: if is_face { 1.0 } else { 0.62 },
                    });
                }
            }
        }

        Self {
            points,
            extent_x: width as f32 / 2.0 + 1.5,
            extent_y: height as f32 / 2.0 + 1.5,
        }
    }
}

fn render_scene(scene: &Scene, angle: f32, width: usize, height: usize) -> Vec<String> {
    let width = width.max(8);
    let height = height.max(5);
    let mut canvas = Canvas::new(width, height);

    let angle_x = angle * 0.63 + 0.28;
    let angle_y = angle;
    let (sin_x, cos_x) = angle_x.sin_cos();
    let (sin_y, cos_y) = angle_y.sin_cos();

    let scale_x = (width as f32 * 0.42) / scene.extent_x.max(0.5);
    let scale_y = (height as f32 * 0.42) / scene.extent_y.max(0.5);
    let light = (-0.35_f32, -0.45_f32, 0.82_f32);

    for point in &scene.points {
        let y1 = point.y * cos_x - point.z * sin_x;
        let z1 = point.y * sin_x + point.z * cos_x;
        let ny1 = point.ny * cos_x - point.nz * sin_x;
        let nz1 = point.ny * sin_x + point.nz * cos_x;

        let x2 = point.x * cos_y + z1 * sin_y;
        let z2 = -point.x * sin_y + z1 * cos_y;
        let nx2 = point.nx * cos_y + nz1 * sin_y;
        let nz2 = -point.nx * sin_y + nz1 * cos_y;

        // A light perspective effect adds depth without making the shape unstable.
        let perspective = 5.0 / (5.0 - z2).max(2.0);
        let screen_x = width as f32 / 2.0 + x2 * scale_x * perspective;
        let screen_y = height as f32 / 2.0 - y1 * scale_y * perspective;

        let diffuse = (nx2 * light.0 + ny1 * light.1 + nz2 * light.2).max(0.0);
        let brightness = (0.28 + diffuse * 0.72) * point.shade;
        canvas.plot(screen_x, screen_y, z2, brightness);
    }

    canvas.lines()
}

fn clean_logo(logo: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in logo.lines() {
        let mut cleaned = String::new();
        let mut chars = raw.chars().peekable();
        while let Some(character) = chars.next() {
            if character == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for code in chars.by_ref() {
                    if code.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                cleaned.push(character);
            }
        }
        lines.push(cleaned.trim_end().to_string());
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

fn char_density(character: char) -> f32 {
    match character {
        ' ' | '\t' => 0.0,
        '.' | '\'' | '\u{60}' => 0.16,
        ':' | ',' | '-' | '_' => 0.28,
        '+' | '=' | '/' | '\\' | '|' => 0.42,
        'o' | 'a' | 'e' | 's' | 'n' => 0.55,
        '█' | 'M' | 'W' | '@' | '#' | '%' => 1.0,
        _ if character.is_ascii_uppercase() => 0.82,
        _ if character.is_ascii_lowercase() => 0.58,
        _ => 0.72,
    }
}

fn logo_for(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();

    // Prefer a specific distro logo before trying its upstream family.
    if name.contains("endeavour") {
        include_str!("../Ascii_art/endeavouros_ascii.txt")
    } else if name.contains("cachy") {
        include_str!("../Ascii_art/cashyos_ascii.txt")
    } else if name.contains("mint") {
        include_str!("../Ascii_art/linuxmint_ascii.txt")
    } else if name.contains("macos") || name.contains("darwin") {
        include_str!("../Ascii_art/macos_ascii.txt")
    } else if name.contains("freebsd") || name.contains("netbsd") {
        include_str!("../Ascii_art/freebsd_ascii.txt")
    } else if name.contains("windows") {
        include_str!("../Ascii_art/windows_ascii.txt")
    } else if name.contains("alpine") {
        include_str!("../Ascii_art/alpine_ascii.txt")
    } else if name.contains("opensuse") || name.contains("suse") || name.contains("tumbleweed") {
        include_str!("../Ascii_art/opensuse_ascii.txt")
    } else if name.contains("nixos") {
        include_str!("../Ascii_art/nixos_ascii.txt")
    } else if name.contains("gentoo") {
        include_str!("../Ascii_art/gentoo_ascii.txt")
    } else if name.contains("manjaro") || name.contains("garuda") || name.contains("artix") {
        include_str!("../Ascii_art/manjaro_ascii.txt")
    } else if name.contains("kali") {
        include_str!("../Ascii_art/kali_ascii.txt")
    } else if name.contains("fedora")
        || name.contains("rhel")
        || name.contains("redhat")
        || name.contains("centos")
        || name.contains("rocky")
        || name.contains("alma")
        || name.contains("oracle")
    {
        include_str!("../Ascii_art/fedora_ascii.txt")
    } else if name.contains("ubuntu")
        || name.contains("pop")
        || name.contains("zorin")
        || name.contains("elementary")
        || name.contains("neon")
    {
        include_str!("../Ascii_art/ubuntu_ascii.txt")
    } else if name.contains("debian")
        || name.contains("mx")
        || name.contains("devuan")
        || name.contains("deepin")
    {
        include_str!("../Ascii_art/debian_ascii.txt")
    } else if name.contains("arch") || name.contains("steam") {
        include_str!("../Ascii_art/arch_ascii.txt")
    } else {
        include_str!("../Ascii_art/default_ascii.txt")
    }
}

#[cfg(test)]
mod tests {
    use super::{Scene, char_density, clean_logo, logo_for};

    #[test]
    fn logo_parser_removes_ansi_codes() {
        let result = clean_logo("\u{1b}[31mABC\u{1b}[0m\n");
        assert_eq!(result, vec!["ABC"]);
    }

    #[test]
    fn whitespace_does_not_become_logo_geometry() {
        assert_eq!(char_density(' '), 0.0);
        assert!(char_density('#') > char_density('.'));
    }

    #[test]
    fn donut_scene_has_geometry() {
        let scene = Scene::donut();
        assert!(!scene.points.is_empty());
        assert!(scene.extent_x > 1.0);
    }

    #[test]
    fn distro_logo_fallbacks_cover_major_families() {
        assert_ne!(logo_for("alpine"), logo_for("unknown-distro"));
        assert_ne!(logo_for("macos"), logo_for("unknown-distro"));
        assert_ne!(logo_for("freebsd"), logo_for("unknown-distro"));
        assert_eq!(logo_for("ubuntu debian"), logo_for("ubuntu"));
        assert_eq!(logo_for("manjaro arch"), logo_for("manjaro"));
    }
}
