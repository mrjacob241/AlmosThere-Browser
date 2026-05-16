/// Inferred browser `screen.*` properties, derived from OS display APIs.
///
/// Mirrors JS: `screen.width`, `screen.height`, `screen.colorDepth`,
/// `screen.pixelDepth`, `screen.availWidth`, `screen.availHeight`.
#[derive(Clone, Debug)]
pub struct ScreenInfo {
    /// `screen.width`  — full horizontal resolution in CSS pixels
    pub width: u32,
    /// `screen.height`  — full vertical resolution in CSS pixels
    pub height: u32,
    /// `screen.colorDepth` / `screen.pixelDepth`  — bits per pixel (usually 24)
    pub color_depth: u32,
    /// `screen.availWidth`  — width excluding OS chrome (taskbar, dock)
    pub avail_width: u32,
    /// `screen.availHeight`  — height excluding OS chrome
    pub avail_height: u32,
}

impl ScreenInfo {
    /// Detect screen properties from the current OS display environment.
    pub fn detect() -> Self {
        let (width, height) = detect_resolution();
        let color_depth = detect_color_depth();
        let (avail_width, avail_height) = detect_avail_area(width, height);
        Self {
            width,
            height,
            color_depth,
            avail_width,
            avail_height,
        }
    }

    /// `screen.pixelDepth` is always identical to `screen.colorDepth` in browsers.
    pub fn pixel_depth(&self) -> u32 {
        self.color_depth
    }
}

// ── Resolution ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn detect_resolution() -> (u32, u32) {
    // Strategy 1: xrandr — parse "1920x1080+0+0" from the connected primary.
    if let Some(res) = xrandr_resolution() {
        return res;
    }
    // Strategy 2: read from /sys/class/drm (kernel DRM, works on Wayland too).
    if let Some(res) = drm_resolution() {
        return res;
    }
    (1920, 1080) // safe fallback
}

#[cfg(target_os = "macos")]
fn detect_resolution() -> (u32, u32) {
    // system_profiler SPDisplaysDataType  →  "Resolution: 2560 x 1600 Retina"
    std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with("Resolution:") {
                    // "Resolution: 2560 x 1600 Retina"
                    let nums: Vec<u32> = t
                        .split_whitespace()
                        .filter_map(|w| w.parse().ok())
                        .collect();
                    if nums.len() >= 2 {
                        return Some((nums[0], nums[1]));
                    }
                }
            }
            None
        })
        .unwrap_or((1440, 900))
}

#[cfg(target_os = "windows")]
fn detect_resolution() -> (u32, u32) {
    // wmic path Win32_VideoController get CurrentHorizontalResolution,CurrentVerticalResolution
    let out = std::process::Command::new("wmic")
        .args([
            "path",
            "Win32_VideoController",
            "get",
            "CurrentHorizontalResolution,CurrentVerticalResolution",
            "/format:value",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let mut w = 0u32;
    let mut h = 0u32;
    for line in out.lines() {
        if let Some(v) = line.strip_prefix("CurrentHorizontalResolution=") {
            w = v.trim().parse().unwrap_or(0);
        }
        if let Some(v) = line.strip_prefix("CurrentVerticalResolution=") {
            h = v.trim().parse().unwrap_or(0);
        }
    }
    if w > 0 && h > 0 { (w, h) } else { (1920, 1080) }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_resolution() -> (u32, u32) {
    (1920, 1080)
}

// ── Linux helpers ──────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn xrandr_resolution() -> Option<(u32, u32)> {
    let out = std::process::Command::new("xrandr")
        .arg("--current")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())?;

    // Look for lines like "   1920x1080     60.03*+" or
    // "eDP-1 connected primary 1920x1080+0+0 ..."
    for line in out.lines() {
        // Connected display with geometry: "1920x1080+0+0"
        if line.contains(" connected") {
            if let Some((w, h)) = parse_wxh_plus(line) {
                return Some((w, h));
            }
        }
        // Active mode line: "   1920x1080     60.03*+"
        if line.trim_start().starts_with(|c: char| c.is_ascii_digit()) {
            let token = line.split_whitespace().next()?;
            if let Some((w, h)) = parse_wxh(token) {
                return Some((w, h));
            }
        }
    }
    None
}

/// Parse "1920x1080+0+0" → (1920, 1080)
#[cfg(target_os = "linux")]
fn parse_wxh_plus(s: &str) -> Option<(u32, u32)> {
    for token in s.split_whitespace() {
        if token.contains('x') && token.contains('+') {
            let base = token.split('+').next()?;
            return parse_wxh(base);
        }
    }
    None
}

/// Parse "1920x1080" → (1920, 1080)
#[cfg(target_os = "linux")]
fn parse_wxh(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.split('x');
    let w: u32 = parts.next()?.parse().ok()?;
    let h: u32 = parts
        .next()?
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()?;
    if w > 0 && h > 0 { Some((w, h)) } else { None }
}

/// Fallback for Wayland / no xrandr: read from kernel DRM connector.
#[cfg(target_os = "linux")]
fn drm_resolution() -> Option<(u32, u32)> {
    // /sys/class/drm/card*/card*-*/modes  — first line is the preferred mode "1920x1080"
    let drm = std::path::Path::new("/sys/class/drm");
    if !drm.exists() {
        return None;
    }
    for entry in std::fs::read_dir(drm).ok()? {
        let path = entry.ok()?.path().join("modes");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(first) = content.lines().next() {
                if let Some(res) = parse_wxh(first.trim()) {
                    return Some(res);
                }
            }
        }
    }
    None
}

// ── Color depth ────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn detect_color_depth() -> u32 {
    // xdpyinfo | grep "default visual" depth
    std::process::Command::new("xdpyinfo")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            for line in s.lines() {
                let t = line.trim();
                if t.starts_with("depth of root window:") {
                    // "depth of root window:    24 planes"
                    return t.split_whitespace().nth(4).and_then(|n| n.parse().ok());
                }
            }
            None
        })
        .unwrap_or(24)
}

#[cfg(not(target_os = "linux"))]
fn detect_color_depth() -> u32 {
    24 // universally 24 on modern macOS and Windows
}

// ── Available area ─────────────────────────────────────────────────────────────
//
// "Available" means the screen minus persistent OS chrome (taskbar, menubar, dock).
// Exact values require platform APIs; we use _NET_WORKAREA on Linux (X11),
// estimate on others.

#[cfg(target_os = "linux")]
fn detect_avail_area(width: u32, height: u32) -> (u32, u32) {
    // X11: _NET_WORKAREA root property  → "0, 0, 1920, 1040"
    if let Some((aw, ah)) = net_workarea() {
        // Clamp: workarea may span virtual desktop > single-screen resolution.
        return (aw.min(width), ah.min(height));
    }
    // Wayland / no xprop: subtract a typical panel height (40 px).
    (width, height.saturating_sub(40))
}

#[cfg(target_os = "linux")]
fn net_workarea() -> Option<(u32, u32)> {
    let out = std::process::Command::new("xprop")
        .args(["-root", "_NET_WORKAREA"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())?;

    // "_NET_WORKAREA(CARDINAL) = 0, 0, 1920, 1040"
    let rhs = out.split('=').nth(1)?;
    let nums: Vec<u32> = rhs
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if nums.len() >= 4 {
        Some((nums[2], nums[3]))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn detect_avail_area(width: u32, height: u32) -> (u32, u32) {
    // macOS always has a 25 px menu bar at top; Dock is usually auto-hidden or
    // at the bottom. Conservative estimate: subtract menu bar only.
    (width, height.saturating_sub(25))
}

#[cfg(target_os = "windows")]
fn detect_avail_area(width: u32, height: u32) -> (u32, u32) {
    // SystemParametersInfo(SPI_GETWORKAREA) would give the exact rect,
    // but requires winapi. Use PowerShell as a zero-dep alternative.
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea | \
                Select-Object -ExpandProperty Height",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok());

    match out {
        Some(avail_h) => (width, avail_h),
        None => (width, height.saturating_sub(40)), // typical taskbar
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_avail_area(width: u32, height: u32) -> (u32, u32) {
    (width, height)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_is_plausible() {
        let s = ScreenInfo::detect();
        assert!(s.width >= 320, "width {} too small", s.width);
        assert!(s.height >= 240, "height {} too small", s.height);
        assert!(s.width <= 7680, "width {} too large", s.width);
        assert!(s.height <= 4320, "height {} too large", s.height);
    }

    #[test]
    fn avail_lte_full() {
        let s = ScreenInfo::detect();
        assert!(s.avail_width <= s.width, "availWidth must be ≤ width");
        assert!(s.avail_height <= s.height, "availHeight must be ≤ height");
    }

    #[test]
    fn avail_is_positive() {
        let s = ScreenInfo::detect();
        assert!(s.avail_width > 0, "availWidth must be > 0");
        assert!(s.avail_height > 0, "availHeight must be > 0");
    }

    #[test]
    fn color_depth_is_standard() {
        let s = ScreenInfo::detect();
        assert!(
            matches!(s.color_depth, 16 | 24 | 30 | 32 | 48),
            "unexpected colorDepth: {}",
            s.color_depth
        );
    }

    #[test]
    fn pixel_depth_equals_color_depth() {
        let s = ScreenInfo::detect();
        assert_eq!(s.pixel_depth(), s.color_depth);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_wxh_works() {
        assert_eq!(super::parse_wxh("1920x1080"), Some((1920, 1080)));
        assert_eq!(super::parse_wxh("2560x1440"), Some((2560, 1440)));
        assert_eq!(super::parse_wxh("3840x2160"), Some((3840, 2160)));
        assert_eq!(super::parse_wxh("bad"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_wxh_plus_works() {
        assert_eq!(
            super::parse_wxh_plus("eDP-1 connected primary 1920x1080+0+0 (normal)"),
            Some((1920, 1080))
        );
    }
}
