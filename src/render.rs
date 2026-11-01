use std::io::{self, IsTerminal, Write};
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
    color: u8,
}

struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<u8>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            cells: vec![b' '; len],
            depth: vec![f32::NEG_INFINITY; len],
        }
    }

    fn resize(&mut self, width: usize, height: usize) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            let len = width.saturating_mul(height);
            self.cells.resize(len, b' ');
            self.depth.resize(len, f32::NEG_INFINITY);
        }
        self.clear();
    }

    fn clear(&mut self) {
        self.cells.fill(b' ');
        self.depth.fill(f32::NEG_INFINITY);
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
    let mut canvas = Canvas::new(1, 1);
    let mut angle = 0.0_f32;
    let mut paused = false;
    let mut next_sample = Instant::now() + SAMPLE_INTERVAL;
    let mut previous_size = None;

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
        if previous_size != Some((columns, rows)) {
            // A full clear is only needed after a resize, never on every animation frame.
            execute!(stdout, Clear(ClearType::All))?;
            previous_size = Some((columns, rows));
        }

        let frame = render_live_frame(
            &scene,
            angle,
            &snapshot,
            columns as usize,
            rows as usize,
            paused,
            &mut canvas,
        );

        // Rewrite rows in place and clear only their tails. This avoids the visible
        // full-screen flash and prevents stale characters after a shorter frame.
        for (row, line) in frame.iter().enumerate() {
            queue!(
                stdout,
                crossterm::cursor::MoveTo(0, row as u16),
                Print(line),
                Clear(ClearType::UntilNewLine)
            )?;
        }
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
    canvas: &mut Canvas,
) -> Vec<String> {
    let columns = columns.max(1);
    let rows = rows.max(1);
    let panel_width = columns.saturating_sub(2).max(12);
    let panel = info_lines(snapshot, panel_width);
    let status = if paused {
        "PAUSED  |  Q quit  Space resume  R refresh"
    } else {
        "LIVE  |  Q quit  Space pause  R refresh"
    };

    let mut output = Vec::with_capacity(rows);

    if columns >= 76 {
        // Even an 80-column terminal can use a side-by-side layout. This keeps
        // the information panel from pushing the artwork below the visible screen.
        let scene_width = (columns / 3).min(48).max(20);
        let scene_height = rows.saturating_sub(1).min(24).max(1);
        let gap = 3;
        let info_width = columns.saturating_sub(scene_width + gap + 1).max(12);
        canvas.resize(scene_width, scene_height);
        let artwork = render_scene(scene, angle, canvas);
        let panel = info_lines(snapshot, info_width);

        for row in 0..rows.saturating_sub(1) {
            let art = artwork.get(row).map(String::as_str).unwrap_or("");
            let info = panel.get(row).map(String::as_str).unwrap_or("");
            let padded_art = format!("{art:<scene_width$}");
            let art = colorize(&padded_art, scene.color, true);
            let info = style_info_line(info);
            output.push(format!("{art}{}{}", " ".repeat(gap), info));
        }
    } else {
        // Small terminals use a stacked layout, but the panel is clipped to the
        // actual remaining rows instead of being allowed to scroll the screen.
        let artwork_room = rows.saturating_sub(panel.len() + 1).min(12);
        let scene_height = artwork_room.max(3);
        let scene_width = columns.saturating_sub(2).max(8).min(52);
        canvas.resize(scene_width, scene_height);
        let artwork = render_scene(scene, angle, canvas);

        for line in artwork.iter().take(rows.saturating_sub(1)) {
            output.push(colorize(line, scene.color, true));
        }
        let remaining = rows.saturating_sub(output.len() + 1);
        for line in panel.iter().take(remaining) {
            output.push(style_info_line(line));
        }
    }

    if output.len() >= rows {
        output.truncate(rows);
        if let Some(last) = output.last_mut() {
            *last = colorize(&truncate(status, columns.saturating_sub(1)), 46, true);
        }
    } else {
        output.push(colorize(&truncate(status, columns.saturating_sub(1)), 46, true));
    }

    output.truncate(rows);
    while output.len() < rows {
        output.push(String::new());
    }
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
        format!("Temp     {}", snapshot.cpu_temperature.map(|value| format!("{value:.0}°C")).unwrap_or_else(|| "N/A".to_string())),
        format!("Disk     {}", snapshot.disk_usage.map(|(used, total)| {
            let percent = percentage(used, total);
            format!("{} / {} ({percent:.0}%)", format_bytes(used), format_bytes(total))
        }).unwrap_or_else(|| "N/A".to_string())),
        format!("GPU      {}", if snapshot.gpu_info.is_empty() { "N/A".to_string() } else { snapshot.gpu_info.join(" | ") }),
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

fn needs_stacked_layout(columns: usize, logo_width: usize, gap: usize, panel_width: usize) -> bool {
    columns < logo_width.saturating_add(gap).saturating_add(panel_width)
}

fn colorize(value: &str, color: u8, bold: bool) -> String {
    if std::env::var_os("NO_COLOR").is_some() || !io::stdout().is_terminal() {
        return value.to_string();
    }
    let intensity = if bold { "1" } else { "0" };
    format!("\x1b[{intensity};38;5;{color}m{value}\x1b[0m")
}

fn ansi_color(value: &str) -> Option<u8> {
    for sequence in value.split("\x1b[").skip(1) {
        let Some(parameters) = sequence.split('m').next() else {
            continue;
        };
        let numbers: Vec<u16> = parameters
            .split(';')
            .filter_map(|part| part.parse::<u16>().ok())
            .collect();

        for (index, code) in numbers.iter().copied().enumerate() {
            if code == 38 && numbers.get(index + 1) == Some(&5) {
                return numbers.get(index + 2).copied().map(|value| value.min(255) as u8);
            }
            if code == 38 && numbers.get(index + 1) == Some(&2) {
                if let (Some(red), Some(green), Some(blue)) = (
                    numbers.get(index + 2),
                    numbers.get(index + 3),
                    numbers.get(index + 4),
                ) {
                    let to_cube = |value: u16| ((value.min(255) * 5 + 127) / 255) as u8;
                    return Some(16 + 36 * to_cube(*red) + 6 * to_cube(*green) + to_cube(*blue));
                }
            }
            if (30..=37).contains(&code) {
                return Some((code - 30) as u8);
            }
            if (90..=97).contains(&code) {
                return Some((code - 90 + 8) as u8);
            }
        }
    }
    None
}

fn logo_color(logo: &str) -> u8 {
    ansi_color(logo).unwrap_or(45)
}

fn style_info_line(line: &str) -> String {
    if line.trim().is_empty() {
        return String::new();
    }
    if line.starts_with("SMARTFETCH") {
        return colorize(line, 45, true);
    }
    if line.starts_with('─') {
        return colorize(line, 240, false);
    }

    let label_end = line.find(char::is_whitespace).unwrap_or(line.len());
    let label = &line[..label_end];
    let rest = &line[label_end..];
    let color = match label {
        "OS" => 39,
        "Host" | "Kernel" => 244,
        "CPU" => 45,
        "Cores" => 81,
        "Temp" => 214,
        "Disk" => 81,
        "GPU" => 141,
        "RAM" => 46,
        "SWAP" => 201,
        "Uptime" => 39,
        "Refresh" => 244,
        _ => 45,
    };
    format!("{}{}", colorize(label, color, true), rest)
}

pub fn render_static(snapshot: &SystemSnapshot, logo_name: &str) -> String {
    let raw_logo = logo_for(logo_name);
    let raw_lines: Vec<&str> = raw_logo.lines().collect();
    let logo_lines = clean_logo(raw_logo);
    let columns = terminal::size().map(|(columns, _)| columns as usize).unwrap_or(80);
    let panel_width = columns.saturating_sub(2).min(64).max(20);
    let panel = info_lines(snapshot, panel_width);
    let logo_width = logo_lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let gap = 6;
    let mut output = String::new();

    if !needs_stacked_layout(columns, logo_width, gap, panel_width) {
        for row in 0..logo_lines.len().max(panel.len()) {
            let plain_art = logo_lines.get(row).map(String::as_str).unwrap_or("");
            let raw_art = raw_lines.get(row).copied().unwrap_or("");
            let color = ansi_color(raw_art).unwrap_or_else(|| logo_color(raw_logo));
            let art = colorize(plain_art, color, true);
            let padding = " ".repeat(logo_width.saturating_sub(plain_art.chars().count()));
            let info = panel.get(row).map(String::as_str).unwrap_or("");
            output.push_str(&art);
            output.push_str(&padding);
            output.push_str(&" ".repeat(gap));
            output.push_str(&style_info_line(info));
            output.push('\n');
        }
    } else {
        for (row, line) in logo_lines.iter().enumerate() {
            let raw_art = raw_lines.get(row).copied().unwrap_or("");
            let color = ansi_color(raw_art).unwrap_or_else(|| logo_color(raw_logo));
            output.push_str(&colorize(line, color, true));
            output.push('\n');
        }
        output.push('\n');
        for line in &panel {
            output.push_str(&style_info_line(line));
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
            color: 45,
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

                let cell_x = column as f32 - width as f32 / 2.0;
                let cell_y = height as f32 / 2.0 - row as f32;
                let depth = 0.25 + density * 1.55;
                let offsets = [-0.28_f32, 0.28_f32];
                let slices = 5;

                // Sample each character cell as a tiny 2x2 height-map tile.
                // Front/back faces and outer edges receive different normals,
                // so the rotating logo reads as a solid relief rather than dots.
                for offset_y in offsets {
                    for offset_x in offsets {
                        for slice in 0..=slices {
                            let t = slice as f32 / slices as f32;
                            let z = -depth / 2.0 + depth * t;
                            let is_front = slice == 0;
                            let is_back = slice == slices;
                            let (nx, ny, nz) = if is_front {
                                (0.0, 0.0, -1.0)
                            } else if is_back {
                                (0.0, 0.0, 1.0)
                            } else if offset_x > 0.0 {
                                (1.0, 0.0, 0.0)
                            } else {
                                (-1.0, 0.0, 0.0)
                            };

                            points.push(Point {
                                x: cell_x + offset_x,
                                y: cell_y + offset_y,
                                z,
                                nx,
                                ny,
                                nz,
                                shade: if is_front || is_back { 1.0 } else { 0.66 },
                            });
                        }
                    }
                }
            }
        }

        Self {
            points,
            extent_x: width as f32 / 2.0 + 1.5,
            extent_y: height as f32 / 2.0 + 1.5,
            color: logo_color(logo),
        }
    }
}

fn render_scene(scene: &Scene, angle: f32, canvas: &mut Canvas) -> Vec<String> {
    canvas.clear();
    let width = canvas.width;
    let height = canvas.height;

    let angle_x = angle * 0.63 + 0.28;
    let angle_y = angle;
    let (sin_x, cos_x) = angle_x.sin_cos();
    let (sin_y, cos_y) = angle_y.sin_cos();

    let scale_x = (width as f32 * 0.42) / scene.extent_x.max(0.5);
    // Terminal cells are roughly twice as tall as they are wide.
    let scale_y = ((width as f32 * 0.21).min(height as f32 * 0.42)) / scene.extent_y.max(0.5);
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
    } else if name.contains("mint") || name.contains("lmde") {
        include_str!("../Ascii_art/linuxmint_ascii.txt")
    } else if name.contains("macos") || name.contains("darwin") {
        include_str!("../Ascii_art/macos_ascii.txt")
    } else if name.contains("netbsd") {
        include_str!("../Ascii_art/netbsd_ascii.txt")
    } else if name.contains("freebsd") {
        include_str!("../Ascii_art/freebsd_ascii.txt")
    } else if name.contains("windows") {
        include_str!("../Ascii_art/windows_ascii.txt")
    } else if name.contains("void") {
        include_str!("../Ascii_art/void_ascii.txt")
    } else if name.contains("solus") {
        include_str!("../Ascii_art/solus_ascii.txt")
    } else if name.contains("mageia") || name.contains("pclinuxos") {
        include_str!("../Ascii_art/mageia_ascii.txt")
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
    use super::{Scene, char_density, clean_logo, logo_for, needs_stacked_layout};

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
    fn ansi_logo_colors_are_detected() {
        assert_eq!(super::ansi_color("\x1b[34mFedora\x1b[0m"), Some(4));
        assert_eq!(super::ansi_color("\x1b[38;5;196mLogo\x1b[0m"), Some(196));
    }

    #[test]
    fn static_layout_keeps_a_real_gutter_or_stacks() {
        assert!(needs_stacked_layout(80, 44, 6, 64));
        assert!(!needs_stacked_layout(120, 44, 6, 64));
    }

    #[test]
    fn live_layout_never_writes_beyond_terminal_height() {
        use crate::system::SystemSnapshot;

        let snapshot = SystemSnapshot {
            os_name: "Fedora Linux".to_string(),
            distro_id: "fedora".to_string(),
            host_name: "test-host".to_string(),
            kernel: "test-kernel".to_string(),
            cpu_name: "test-cpu".to_string(),
            logical_cores: 4,
            physical_cores: 2,
            cpu_usage: 12.5,
            cpu_temperature: None,
            disk_usage: None,
            gpu_info: Vec::new(),
            used_memory: 512,
            total_memory: 1024,
            used_swap: 0,
            total_swap: 1024,
            uptime_seconds: 60,
        };
        let scene = Scene::donut();
        let mut canvas = super::Canvas::new(1, 1);

        let wide = super::render_live_frame(&scene, 0.0, &snapshot, 80, 24, false, &mut canvas);
        assert_eq!(wide.len(), 24);

        let narrow = super::render_live_frame(&scene, 0.0, &snapshot, 52, 24, false, &mut canvas);
        assert_eq!(narrow.len(), 24);

        let short = super::render_live_frame(&scene, 0.0, &snapshot, 52, 10, false, &mut canvas);
        assert_eq!(short.len(), 10);
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
