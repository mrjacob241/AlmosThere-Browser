/// Mockup implementations for amiunique.org fingerprint attributes that cannot
/// be read from simple OS APIs.
///
/// Each struct documents what a real browser collects and returns either a
/// detected value (where the OS gives us a handle) or a deterministic mock
/// (where full browser APIs like canvas rendering or audio DSP are required).
///
/// Sections map to AMIUNIQUE_fp.md:
///   §6  Canvas               → CanvasFingerprint
///   §7  WebGL                → WebGlInfo
///   §8  Audio                → AudioFingerprint
///   §9  Fonts                → FontList
///   §10 Touch                → TouchInfo
///   §11 API overwrite        → OverwriteInfo
///   §12 Navigator prototype  → NavigatorPrototypeInfo
///   §13 Math / JIT           → MathConstants
///   §14 Error shape          → ErrorShapeInfo
///   §15 Stack depth          → StackDepthInfo
///   §16 Modernizr            → ModernizrFlags
///   §17 OS media queries     → OsMediaQueries
///   §19 Unknown image error  → UnknownImageInfo
///   §4  Timezone             → TimezoneInfo
///   §5  Storage              → StorageInfo
///   §18 AdBlock              → (bool field on FingerprintSuite)

#[cfg(unix)]
use libc;

// ── §6  Canvas ─────────────────────────────────────────────────────────────────

/// `canvas.toDataURL()` fingerprint.
///
/// A real browser renders an orange rectangle plus two text strings with
/// specific fonts, then hashes the PNG data URL.  Without a rendering pipeline
/// we return a deterministic placeholder.
#[derive(Clone, Debug)]
pub struct CanvasFingerprint {
    /// Placeholder that signals "no real canvas render available yet".
    /// A real value looks like a long base64 PNG data URL.
    pub data_url: String,
    /// Whether `HTMLCanvasElement.prototype.toDataURL` appears native
    /// (i.e. not replaced by a browser extension).  Always true here.
    pub is_native: bool,
}

impl CanvasFingerprint {
    pub fn detect() -> Self {
        let data_url = format!(
            "data:image/png;base64,ALMOSTTHERE-MOCK-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        Self {
            data_url,
            is_native: true,
        }
    }
}

// ── §7  WebGL ──────────────────────────────────────────────────────────────────

/// WebGL vendor / renderer strings and a representative parameter sweep.
///
/// In a real browser: `gl.getParameter(gl.VENDOR)` / `gl.getParameter(gl.RENDERER)`.
/// We read the GPU info from the OS where possible.
#[derive(Clone, Debug)]
pub struct WebGlInfo {
    /// `gl.getParameter(gl.VENDOR)` — e.g. `"Intel Open Source Technology Center"`
    pub vendor: String,
    /// `gl.getParameter(gl.RENDERER)` — e.g. `"Mesa Intel(R) UHD Graphics 620"`
    pub renderer: String,
    /// `gl.getParameter(gl.VERSION)` string
    pub version: String,
    /// `gl.getParameter(gl.SHADING_LANGUAGE_VERSION)` string
    pub shading_language_version: String,
    /// Key GL parameters (constant name → string value).
    /// A real sweep covers ~60 constants; we expose the most fingerprint-relevant ones.
    pub parameters: Vec<(&'static str, String)>,
    /// `false` if no GL driver was found (software or headless).
    pub is_supported: bool,
}

impl WebGlInfo {
    pub fn detect() -> Self {
        let (vendor, renderer) = detect_gl_strings();
        let supported = !vendor.is_empty();
        let version = if supported {
            format!("WebGL 1.0 (OpenGL {vendor})")
        } else {
            String::new()
        };
        let slv = if supported {
            "WebGL GLSL ES 1.0".to_owned()
        } else {
            String::new()
        };
        let parameters = if supported {
            vec![
                ("VENDOR", vendor.clone()),
                ("RENDERER", renderer.clone()),
                ("MAX_TEXTURE_SIZE", "16384".into()),
                ("MAX_VIEWPORT_DIMS", "32767,32767".into()),
                ("MAX_VERTEX_ATTRIBS", "16".into()),
                ("MAX_VERTEX_UNIFORM_VECTORS", "4096".into()),
                ("MAX_FRAGMENT_UNIFORM_VECTORS", "1024".into()),
                ("MAX_VARYING_VECTORS", "32".into()),
                ("MAX_COMBINED_TEXTURE_IMAGE_UNITS", "96".into()),
                ("MAX_VERTEX_TEXTURE_IMAGE_UNITS", "32".into()),
                ("MAX_TEXTURE_IMAGE_UNITS", "32".into()),
                ("MAX_CUBE_MAP_TEXTURE_SIZE", "16384".into()),
                ("MAX_RENDERBUFFER_SIZE", "16384".into()),
                ("ALIASED_LINE_WIDTH_RANGE", "1,1".into()),
                ("ALIASED_POINT_SIZE_RANGE", "1,8192".into()),
                ("RED_BITS", "8".into()),
                ("GREEN_BITS", "8".into()),
                ("BLUE_BITS", "8".into()),
                ("ALPHA_BITS", "8".into()),
                ("DEPTH_BITS", "24".into()),
                ("STENCIL_BITS", "8".into()),
                ("SAMPLE_BUFFERS", "0".into()),
                ("SAMPLES", "0".into()),
            ]
        } else {
            vec![]
        };
        Self {
            vendor,
            renderer,
            version,
            shading_language_version: slv,
            parameters,
            is_supported: supported,
        }
    }
}

#[cfg(target_os = "linux")]
fn detect_gl_strings() -> (String, String) {
    glxinfo_strings().unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn glxinfo_strings() -> Option<(String, String)> {
    let out = std::process::Command::new("glxinfo")
        .arg("-B")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())?;

    let mut vendor = None;
    let mut renderer = None;
    for line in out.lines() {
        let t = line.trim();
        if t.starts_with("OpenGL vendor string:") {
            vendor = Some(
                t.trim_start_matches("OpenGL vendor string:")
                    .trim()
                    .to_owned(),
            );
        } else if t.starts_with("OpenGL renderer string:") {
            renderer = Some(
                t.trim_start_matches("OpenGL renderer string:")
                    .trim()
                    .to_owned(),
            );
        }
        if vendor.is_some() && renderer.is_some() {
            break;
        }
    }
    Some((vendor?, renderer?))
}

#[cfg(target_os = "macos")]
fn detect_gl_strings() -> (String, String) {
    let renderer = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            s.lines()
                .find(|l| l.trim_start().starts_with("Chipset Model:"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_else(|| "Apple GPU".to_owned());
    ("Apple".into(), renderer)
}

#[cfg(target_os = "windows")]
fn detect_gl_strings() -> (String, String) {
    let renderer = std::process::Command::new("wmic")
        .args(["path", "Win32_VideoController", "get", "Name", "/value"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Name="))
                .map(|l| l.trim_start_matches("Name=").trim().to_owned())
        })
        .unwrap_or_else(|| "Unknown GPU".to_owned());
    ("Microsoft".into(), renderer)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_gl_strings() -> (String, String) {
    (String::new(), String::new())
}

// ── §8  Audio ──────────────────────────────────────────────────────────────────

/// Four-method Web Audio fingerprint (PXI, NT-VC, CC, Hybrid).
///
/// All values are mocked — real values require an `OfflineAudioContext` DSP run.
/// The constants below are documented typical Chrome values used in fingerprint
/// research, chosen to be self-consistent and plausible.
#[derive(Clone, Debug)]
pub struct AudioFingerprint {
    /// PXI method: sum of `Math.abs(sample)` for samples 4500-5000 from an
    /// `OfflineAudioContext` triangle oscillator through a DynamicsCompressor.
    pub pxi_sum: f64,
    /// NT-VC method: sorted string of all numeric AudioContext / AnalyserNode
    /// property values joined with `;`.
    pub nt_vc_props: String,
    /// CC method: first 30 frequency bins from `getFloatFrequencyData()` (dB values).
    pub cc_bins: Vec<f32>,
    /// Hybrid method sum (CC + compressor variant of PXI).
    pub hybrid_sum: f64,
    /// `false` if the audio subsystem is unavailable (headless / no ALSA).
    pub is_supported: bool,
}

impl AudioFingerprint {
    pub fn detect() -> Self {
        let supported = detect_audio_supported();
        if !supported {
            return Self {
                pxi_sum: 0.0,
                nt_vc_props: String::new(),
                cc_bins: vec![],
                hybrid_sum: 0.0,
                is_supported: false,
            };
        }
        // Mock values representative of a Chrome 120 Linux/x86_64 baseline.
        let pxi_sum = 124.043_449_684_750_72;
        let nt_vc_props = concat!(
            "bufferSize:4096;channelCount:2;channelCountMode:explicit;",
            "channelInterpretation:speakers;fftSize:2048;maxChannelCount:2;",
            "numberOfInputs:0;numberOfOutputs:1;sampleRate:44100.0"
        )
        .to_owned();
        // Typical getFloatFrequencyData() values (all negative dB, noise floor around -100).
        let cc_bins: Vec<f32> = (0..30).map(|i| -99.9 + (i as f32) * 0.3).collect();
        let hybrid_sum = 124.043_449_684_750_72;

        Self {
            pxi_sum,
            nt_vc_props,
            cc_bins,
            hybrid_sum,
            is_supported: true,
        }
    }
}

#[cfg(target_os = "linux")]
fn detect_audio_supported() -> bool {
    // Minimal check: ALSA or PulseAudio device exists.
    std::path::Path::new("/dev/snd").exists() || std::path::Path::new("/run/user").exists()
}

#[cfg(not(target_os = "linux"))]
fn detect_audio_supported() -> bool {
    true // assume audio is available on macOS/Windows
}

// ── §9  Fonts ──────────────────────────────────────────────────────────────────

/// Font availability via CSS dimension probing.
///
/// amiunique.org renders test text in ~500 fonts against monospace/sans/serif
/// baselines.  We query the OS font catalog instead (no rendering required).
#[derive(Clone, Debug)]
pub struct FontList {
    /// Font family names detected as installed on the system.
    pub installed: Vec<String>,
}

impl FontList {
    pub fn detect() -> Self {
        let installed = detect_fonts();
        Self { installed }
    }

    /// Format as the amiunique `fontsEnum` string: `"FontA--true;FontB--false;..."`
    /// for a fixed probe list.  Only a representative subset is included.
    pub fn as_amiunique_string(&self) -> String {
        let probe: &[&str] = AMIUNIQUE_PROBE_FONTS;
        let set: std::collections::HashSet<String> =
            self.installed.iter().map(|s| s.to_lowercase()).collect();
        probe
            .iter()
            .map(|f| format!("{}--{}", f, set.contains(&f.to_lowercase())))
            .collect::<Vec<_>>()
            .join(";")
    }
}

/// Representative subset of the ~500 fonts amiunique.org probes.
const AMIUNIQUE_PROBE_FONTS: &[&str] = &[
    "Arial",
    "Arial Black",
    "Arial Narrow",
    "Arial Rounded MT Bold",
    "Calibri",
    "Cambria",
    "Candara",
    "Comic Sans MS",
    "Consolas",
    "Constantia",
    "Corbel",
    "Courier",
    "Courier New",
    "Georgia",
    "Impact",
    "Lucida Console",
    "Lucida Sans Unicode",
    "Microsoft Sans Serif",
    "Palatino Linotype",
    "Segoe UI",
    "Tahoma",
    "Times",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
    // macOS
    "Helvetica",
    "Helvetica Neue",
    "Monaco",
    "Optima",
    "Futura",
    "Gill Sans",
    "Hoefler Text",
    // Linux common
    "DejaVu Sans",
    "DejaVu Serif",
    "DejaVu Sans Mono",
    "Liberation Sans",
    "Liberation Serif",
    "Liberation Mono",
    "Ubuntu",
    "Ubuntu Mono",
    "Noto Sans",
    "Noto Serif",
    "FreeSans",
    "FreeSerif",
    "FreeMono",
    // CJK
    "MS Gothic",
    "MS Mincho",
    "MingLiU",
    "SimSun",
    "SimHei",
    "NSimSun",
    "FangSong",
    "KaiTi",
    "PMingLiU",
];

#[cfg(target_os = "linux")]
fn detect_fonts() -> Vec<String> {
    let out = std::process::Command::new("fc-list")
        .args([":", "family"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let mut families: Vec<String> = out
        .lines()
        .flat_map(|line| line.split(','))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();

    families.sort_unstable();
    families.dedup();
    families
}

#[cfg(target_os = "macos")]
fn detect_fonts() -> Vec<String> {
    // system_profiler SPFontsDataType is slow; use fc-list if installed,
    // otherwise fall back to a known macOS baseline.
    if let Ok(o) = std::process::Command::new("fc-list")
        .args([":", "family"])
        .output()
    {
        if let Ok(s) = String::from_utf8(o.stdout) {
            if !s.is_empty() {
                let mut v: Vec<String> = s
                    .lines()
                    .flat_map(|l| l.split(','))
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                v.sort_unstable();
                v.dedup();
                return v;
            }
        }
    }
    // Baseline macOS system fonts
    vec![
        "Helvetica".into(),
        "Helvetica Neue".into(),
        "Geneva".into(),
        "Monaco".into(),
        "Times".into(),
        "Courier".into(),
        "Courier New".into(),
        "Arial".into(),
        "Verdana".into(),
        "Georgia".into(),
        "Optima".into(),
        "Palatino".into(),
        "Gill Sans".into(),
        "Futura".into(),
    ]
}

#[cfg(target_os = "windows")]
fn detect_fonts() -> Vec<String> {
    // HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts
    // Easiest cross-version approach: PowerShell
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "[System.Drawing.FontFamily]::Families | Select-Object -ExpandProperty Name",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let mut v: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect_fonts() -> Vec<String> {
    vec![]
}

// ── §10 Touch ──────────────────────────────────────────────────────────────────

/// Touch support as `"maxPoints;touchEventExists;ontouchstartExists"`.
#[derive(Clone, Debug)]
pub struct TouchInfo {
    pub max_touch_points: u32,
    pub touch_event_exists: bool,
    pub ontouchstart_exists: bool,
}

impl TouchInfo {
    pub fn detect() -> Self {
        // Desktop OS with no touch screen: all false / zero.
        Self {
            max_touch_points: 0,
            touch_event_exists: false,
            ontouchstart_exists: false,
        }
    }

    pub fn as_amiunique_string(&self) -> String {
        format!(
            "{};{};{}",
            self.max_touch_points, self.touch_event_exists, self.ontouchstart_exists,
        )
    }
}

// ── §11 API overwrite detection ────────────────────────────────────────────────

/// Checks whether key browser APIs have been replaced by extensions / privacy tools.
///
/// A real browser calls `.toString()` on the native getter/function and checks
/// for `[native code]`.  We always report native since we are the implementation.
#[derive(Clone, Debug)]
pub struct OverwriteInfo {
    /// `Object.getOwnPropertyDescriptor(Object.getPrototypeOf(screen), "width").get.toString()`
    pub screen_width_getter: String,
    /// `HTMLCanvasElement.prototype.toDataURL.toString()`
    pub canvas_to_data_url: String,
    /// `Date.prototype.getTimezoneOffset.toString()`
    pub date_get_timezone_offset: String,
}

impl OverwriteInfo {
    pub fn detect() -> Self {
        Self {
            screen_width_getter: "function get width() { [native code] }".into(),
            canvas_to_data_url: "function toDataURL() { [native code] }".into(),
            date_get_timezone_offset: "function getTimezoneOffset() { [native code] }".into(),
        }
    }

    pub fn all_native(&self) -> bool {
        [
            &self.screen_width_getter,
            &self.canvas_to_data_url,
            &self.date_get_timezone_offset,
        ]
        .iter()
        .all(|s| s.contains("[native code]"))
    }
}

// ── §12 Navigator prototype walk ───────────────────────────────────────────────

/// All property names reachable by walking `navigator`'s prototype chain.
///
/// A real browser yields ~60-80 names.  We return the standard Chrome set.
#[derive(Clone, Debug)]
pub struct NavigatorPrototypeInfo {
    pub properties: Vec<&'static str>,
}

impl NavigatorPrototypeInfo {
    pub fn detect() -> Self {
        Self {
            properties: vec![
                // Own properties on the Navigator object
                "appCodeName",
                "appName",
                "appVersion",
                "platform",
                "userAgent",
                "cookieEnabled",
                "doNotTrack",
                "hardwareConcurrency",
                "language",
                "languages",
                "maxTouchPoints",
                "mimeTypes",
                "onLine",
                "plugins",
                "product",
                "productSub",
                "vendor",
                "vendorSub",
                // NavigatorID
                "javaEnabled",
                "taintEnabled",
                // NavigatorConcurrentHardware (already above)
                // NavigatorContentUtils
                "registerProtocolHandler",
                // NavigatorCookies (already above)
                // NavigatorLanguage (already above)
                // NavigatorNetworkInformation
                "connection",
                // NavigatorOnLine (already above)
                // NavigatorPlugins (already above)
                // NavigatorStorage
                "storage",
                // NavigatorUserActivation
                "userActivation",
                // Additional Chrome extras
                "bluetooth",
                "clipboard",
                "credentials",
                "geolocation",
                "keyboard",
                "locks",
                "mediaCapabilities",
                "mediaDevices",
                "mediaSession",
                "permissions",
                "presentation",
                "requestMIDIAccess",
                "requestMediaKeySystemAccess",
                "sendBeacon",
                "serviceWorker",
                "share",
                "usb",
                "vibrate",
                "wakeLock",
                "webdriver",
                "xr",
                "getBattery",
                "getGamepads",
            ],
        }
    }
}

// ── §13 Math / JIT constants ───────────────────────────────────────────────────

/// Floating-point results of transcendental functions.
///
/// JS engines differ in the last few ULP of these due to JIT / libm differences.
/// We compute directly in Rust (IEEE 754 double); on x86_64 these match V8/SpiderMonkey.
#[derive(Clone, Debug)]
pub struct MathConstants {
    pub asinh_1: f64,        // Math.asinh(1)
    pub acosh_1e300: String, // Math.acosh(1e300) — "Infinity" in old Chromium, finite in V8 120+
    pub atanh_half: f64,     // Math.atanh(0.5)
    pub expm1_1: f64,        // Math.expm1(1)
    pub cbrt_100: f64,       // Math.cbrt(100)
    pub log1p_10: f64,       // Math.log1p(10)
    pub sinh_1: f64,         // Math.sinh(1)
    pub cosh_10: f64,        // Math.cosh(10)
    pub tanh_1: f64,         // Math.tanh(1)
}

impl MathConstants {
    pub fn detect() -> Self {
        let acosh_1e300 = {
            let v = 1e300_f64.acosh();
            if v.is_infinite() {
                "Infinity".to_owned()
            } else {
                v.to_string()
            }
        };
        Self {
            asinh_1: 1.0_f64.asinh(),
            acosh_1e300,
            atanh_half: 0.5_f64.atanh(),
            expm1_1: 1.0_f64.exp_m1(),
            cbrt_100: 100.0_f64.cbrt(),
            log1p_10: 10.0_f64.ln_1p(),
            sinh_1: 1.0_f64.sinh(),
            cosh_10: 10.0_f64.cosh(),
            tanh_1: 1.0_f64.tanh(),
        }
    }
}

// ── §14 Error shape ────────────────────────────────────────────────────────────

/// Browser-specific error object properties.
///
/// amiunique catches a deliberate `ReferenceError` and a bad `WebSocket` to probe
/// which extra properties (fileName, lineNumber, toSource, description) are present.
/// We emulate Chrome, which only exposes `name` and `message`.
#[derive(Clone, Debug)]
pub struct ErrorShapeInfo {
    // ReferenceError
    pub ref_name: String,
    pub ref_message: String,
    pub ref_file_name: Option<String>,   // Firefox-only
    pub ref_line_number: Option<u32>,    // Firefox-only
    pub ref_description: Option<String>, // IE-only
    pub ref_to_source: Option<String>,   // Firefox-only
    // WebSocket error
    pub ws_name: String,
    pub ws_message: String,
}

impl ErrorShapeInfo {
    pub fn detect() -> Self {
        // Chrome format for a bare `not_defined` reference
        Self {
            ref_name: "ReferenceError".into(),
            ref_message: "not_defined is not defined".into(),
            ref_file_name: None,
            ref_line_number: None,
            ref_description: None,
            ref_to_source: None,
            ws_name: "TypeError".into(),
            ws_message: "Failed to construct 'WebSocket': 1 argument required, but only 0 present."
                .into(),
        }
    }
}

// ── §15 Stack overflow depth ───────────────────────────────────────────────────

/// Maximum recursion depth before a `RangeError` is thrown.
///
/// Chrome ~120 on x86_64 Linux typically reaches ~14 370.
/// Firefox reaches ~26 000. We report a Chrome-like value.
#[derive(Clone, Debug)]
pub struct StackDepthInfo {
    pub depth: u32,
    pub error_name: String,
    pub error_message: String,
}

impl StackDepthInfo {
    pub fn detect() -> Self {
        let depth = match std::env::consts::OS {
            "windows" => 13_982,
            "macos" => 15_662,
            _ => 14_370, // Linux x86_64 Chrome baseline
        };
        Self {
            depth,
            error_name: "RangeError".into(),
            error_message: "Maximum call stack size exceeded".into(),
        }
    }
}

// ── §16 Modernizr flags ────────────────────────────────────────────────────────

/// Boolean Modernizr feature-detection results.
///
/// Values reflect what AlmosThere Browser currently supports or plans to support.
#[derive(Clone, Debug)]
pub struct ModernizrFlags {
    pub canvas: bool,
    pub canvas_text: bool,
    pub webgl: bool,
    pub touch: bool,
    pub audio: bool, // HTMLAudioElement
    pub video: bool, // HTMLVideoElement
    pub flexbox: bool,
    pub css_animations: bool,
    pub css_transforms: bool,
    pub css_transforms_3d: bool,
    pub css_transitions: bool,
    pub css_gradients: bool,
    pub border_radius: bool,
    pub box_shadow: bool,
    pub text_shadow: bool,
    pub rgba: bool,
    pub hsla: bool,
    pub opacity: bool,
    pub local_storage: bool,
    pub session_storage: bool,
    pub web_workers: bool,
    pub web_sockets: bool,
    pub geolocation: bool,
    pub svg: bool,
    pub inline_svg: bool,
    pub svg_clip_paths: bool,
    pub smil: bool,
    pub font_face: bool,
    pub hash_change: bool,
    pub history: bool,
    pub post_message: bool,
    pub indexed_db: bool,
    pub application_cache: bool,
    pub drag_and_drop: bool,
    pub generated_content: bool,
    pub multiple_bgs: bool,
    pub background_size: bool,
    pub border_image: bool,
    pub css_columns: bool,
    pub css_reflections: bool,
    pub web_sql: bool,
}

impl ModernizrFlags {
    pub fn detect() -> Self {
        let webgl = WebGlInfo::detect().is_supported;
        let audio_supported = detect_audio_supported();
        Self {
            canvas: true,
            canvas_text: true,
            webgl,
            touch: false, // desktop, non-touch
            audio: audio_supported,
            video: false, // not yet implemented
            flexbox: true,
            css_animations: true,
            css_transforms: true,
            css_transforms_3d: true,
            css_transitions: true,
            css_gradients: true,
            border_radius: true,
            box_shadow: true,
            text_shadow: true,
            rgba: true,
            hsla: true,
            opacity: true,
            local_storage: true,
            session_storage: true,
            web_workers: false, // not yet implemented
            web_sockets: false, // not yet implemented
            geolocation: false, // requires user permission
            svg: true,
            inline_svg: true,
            svg_clip_paths: true,
            smil: false,
            font_face: true,
            hash_change: true,
            history: true,
            post_message: true,
            indexed_db: false,
            application_cache: false,
            drag_and_drop: true,
            generated_content: true,
            multiple_bgs: true,
            background_size: true,
            border_image: true,
            css_columns: true,
            css_reflections: false,
            web_sql: false,
        }
    }

    /// Format as `"flagName--value"` pairs joined by `;`, matching amiunique's output.
    pub fn as_amiunique_string(&self) -> String {
        macro_rules! flag {
            ($name:literal, $val:expr) => {
                format!("{}--{}", $name, $val)
            };
        }
        vec![
            flag!("canvas", self.canvas),
            flag!("canvastext", self.canvas_text),
            flag!("webgl", self.webgl),
            flag!("touch", self.touch),
            flag!("audio", self.audio),
            flag!("video", self.video),
            flag!("flexbox", self.flexbox),
            flag!("cssanimations", self.css_animations),
            flag!("csstransforms", self.css_transforms),
            flag!("csstransforms3d", self.css_transforms_3d),
            flag!("csstransitions", self.css_transitions),
            flag!("cssgradients", self.css_gradients),
            flag!("borderradius", self.border_radius),
            flag!("boxshadow", self.box_shadow),
            flag!("text-shadow", self.text_shadow),
            flag!("rgba", self.rgba),
            flag!("hsla", self.hsla),
            flag!("opacity", self.opacity),
            flag!("localstorage", self.local_storage),
            flag!("sessionstorage", self.session_storage),
            flag!("webworkers", self.web_workers),
            flag!("websockets", self.web_sockets),
            flag!("geolocation", self.geolocation),
            flag!("svg", self.svg),
            flag!("inlinesvg", self.inline_svg),
            flag!("svgclippaths", self.svg_clip_paths),
            flag!("smil", self.smil),
            flag!("fontface", self.font_face),
            flag!("hashchange", self.hash_change),
            flag!("history", self.history),
            flag!("postmessage", self.post_message),
            flag!("indexeddb", self.indexed_db),
            flag!("applicationcache", self.application_cache),
            flag!("draganddrop", self.drag_and_drop),
            flag!("generatedcontent", self.generated_content),
            flag!("multiplebgs", self.multiple_bgs),
            flag!("backgroundsize", self.background_size),
            flag!("borderimage", self.border_image),
            flag!("csscolumns", self.css_columns),
            flag!("cssreflections", self.css_reflections),
            flag!("websqldatabase", self.web_sql),
        ]
        .join(";")
    }
}

// ── §17 OS media queries ───────────────────────────────────────────────────────

/// Results of the five OS-targeting CSS media queries amiunique injects.
///
/// The page embeds styles that set `color: red` on `#testmac1`, `#testwinxp`,
/// `#testwinvis`, `#testwin7`, `#testwin8` for the matching OS.
/// We infer the result from `std::env::consts::OS`.
#[derive(Clone, Debug)]
pub struct OsMediaQueries {
    pub mac: bool,
    pub win_xp: bool,
    pub win_vista: bool,
    pub win7: bool,
    pub win8: bool,
}

impl OsMediaQueries {
    pub fn detect() -> Self {
        match std::env::consts::OS {
            "macos" => Self {
                mac: true,
                win_xp: false,
                win_vista: false,
                win7: false,
                win8: false,
            },
            "windows" => Self::for_windows(),
            _ => Self {
                mac: false,
                win_xp: false,
                win_vista: false,
                win7: false,
                win8: false,
            },
        }
    }

    #[cfg(target_os = "windows")]
    fn for_windows() -> Self {
        // Parse major.minor from os_version to decide which CSS query fires.
        let ver = detect_windows_nt_version();
        let (win_xp, win_vista, win7, win8) = match ver.as_deref() {
            Some("5.1") | Some("5.2") => (true, false, false, false), // XP
            Some("6.0") => (false, true, false, false),               // Vista
            Some("6.1") => (false, false, true, false),               // 7
            Some("6.2") | Some("6.3") => (false, false, false, true), // 8 / 8.1
            _ => (false, false, false, false),                        // 10+
        };
        Self {
            mac: false,
            win_xp,
            win_vista,
            win7,
            win8,
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn for_windows() -> Self {
        Self {
            mac: false,
            win_xp: false,
            win_vista: false,
            win7: false,
            win8: false,
        }
    }

    pub fn as_amiunique_string(&self) -> String {
        format!(
            "{}/{}/{}/{}/{}",
            self.mac, self.win_xp, self.win_vista, self.win7, self.win8
        )
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_nt_version() -> Option<String> {
    let out = std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())?;
    let start = out.find("Version ")? + "Version ".len();
    let rest = &out[start..];
    let end = rest.find(']').unwrap_or(rest.len());
    let full = rest[..end].trim();
    // "10.0.19041" → "10.0"
    let parts: Vec<&str> = full.splitn(3, '.').collect();
    if parts.len() >= 2 {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}

// ── §19 Unknown image error ────────────────────────────────────────────────────

/// `width;height` of a broken `<img>` element after a 1-second load attempt.
///
/// Chrome and Firefox both return `0;0` for images that fail to load.
#[derive(Clone, Debug)]
pub struct UnknownImageInfo {
    pub width: u32,
    pub height: u32,
}

impl UnknownImageInfo {
    pub fn detect() -> Self {
        Self {
            width: 0,
            height: 0,
        }
    }

    pub fn as_amiunique_string(&self) -> String {
        format!("{};{}", self.width, self.height)
    }
}

// ── §4  Timezone ───────────────────────────────────────────────────────────────

/// `new Date().getTimezoneOffset()` — minutes west of UTC.
///
/// UTC+2 → `-120`, UTC-5 → `+300`.  JS uses the inverted sign convention.
#[derive(Clone, Debug)]
pub struct TimezoneInfo {
    pub offset_minutes: i32,
    pub iana_name: Option<String>,
}

impl TimezoneInfo {
    pub fn detect() -> Self {
        let (offset, iana) = detect_timezone();
        Self {
            offset_minutes: offset,
            iana_name: iana,
        }
    }
}

fn detect_timezone() -> (i32, Option<String>) {
    (tz_offset_minutes(), iana_timezone_name())
}

// ── UTC offset via syscall ─────────────────────────────────────────────────────

#[cfg(unix)]
fn tz_offset_minutes() -> i32 {
    // POSIX localtime_r populates tm_gmtoff (seconds east of UTC).
    // libc handles all platform-specific struct tm layout differences.
    let now: libc::time_t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as libc::time_t;

    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&now, &mut tm).is_null() };
    if !ok {
        return 0;
    }

    // tm_gmtoff = seconds east of UTC; JS getTimezoneOffset() = minutes WEST
    let gmtoff: i64 = tm.tm_gmtoff as i64;
    (-(gmtoff / 60)) as i32
}

#[cfg(target_os = "windows")]
fn tz_offset_minutes() -> i32 {
    // GetTimeZoneInformation — Bias is already in JS convention (minutes west of UTC).
    #[repr(C)]
    #[allow(non_snake_case, dead_code)]
    struct SYSTEMTIME {
        wYear: u16,
        wMonth: u16,
        wDayOfWeek: u16,
        wDay: u16,
        wHour: u16,
        wMinute: u16,
        wSecond: u16,
        wMilliseconds: u16,
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct TIME_ZONE_INFORMATION {
        Bias: i32,
        StandardName: [u16; 32],
        StandardDate: SYSTEMTIME,
        StandardBias: i32,
        DaylightName: [u16; 32],
        DaylightDate: SYSTEMTIME,
        DaylightBias: i32,
    }
    extern "system" {
        fn GetTimeZoneInformation(lpTZI: *mut TIME_ZONE_INFORMATION) -> u32;
    }
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
    let mut tzi: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    let result = unsafe { GetTimeZoneInformation(&mut tzi) };
    if result == TIME_ZONE_ID_DAYLIGHT {
        tzi.Bias + tzi.DaylightBias
    } else {
        tzi.Bias + tzi.StandardBias
    }
}

#[cfg(not(any(unix, target_os = "windows")))]
fn tz_offset_minutes() -> i32 {
    0
}

// ── IANA timezone name ─────────────────────────────────────────────────────────

fn iana_timezone_name() -> Option<String> {
    // Strategy 1 (Linux/macOS): /etc/localtime is a symlink into zoneinfo/.
    if let Ok(link) = std::fs::read_link("/etc/localtime") {
        let s = link.to_string_lossy();
        if let Some(i) = s.find("zoneinfo/") {
            return Some(s[i + "zoneinfo/".len()..].to_owned());
        }
    }

    // Strategy 2 (Debian/Ubuntu): /etc/timezone is a plain-text IANA name.
    if let Ok(content) = std::fs::read_to_string("/etc/timezone") {
        let name = content.trim().to_owned();
        if !name.is_empty() {
            return Some(name);
        }
    }

    // Strategy 3 (Windows): GetDynamicTimeZoneInformation returns a Windows tz
    // name (e.g. "W. Europe Standard Time").  We return it as-is; IANA mapping
    // would require an embedded table that is out of scope for a mockup.
    #[cfg(target_os = "windows")]
    {
        if let Some(name) = windows_tz_name() {
            return Some(name);
        }
    }

    // Strategy 4: TZ environment variable (portable fallback).
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.is_empty() {
            return Some(tz);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_tz_name() -> Option<String> {
    // GetDynamicTimeZoneInformation gives the Windows timezone key name.
    #[repr(C)]
    #[allow(non_snake_case, dead_code)]
    struct SYSTEMTIME {
        wYear: u16,
        wMonth: u16,
        wDayOfWeek: u16,
        wDay: u16,
        wHour: u16,
        wMinute: u16,
        wSecond: u16,
        wMilliseconds: u16,
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct DYNAMIC_TIME_ZONE_INFORMATION {
        Bias: i32,
        StandardName: [u16; 32],
        StandardDate: SYSTEMTIME,
        StandardBias: i32,
        DaylightName: [u16; 32],
        DaylightDate: SYSTEMTIME,
        DaylightBias: i32,
        TimeZoneKeyName: [u16; 128],
        DynamicDaylightTimeDisabled: u8,
    }
    extern "system" {
        fn GetDynamicTimeZoneInformation(
            pTimeZoneInformation: *mut DYNAMIC_TIME_ZONE_INFORMATION,
        ) -> u32;
    }
    let mut dtzi: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    let r = unsafe { GetDynamicTimeZoneInformation(&mut dtzi) };
    if r == u32::MAX {
        return None;
    } // ERROR_FILE_NOT_FOUND
    // Decode UTF-16 key name (null-terminated).
    let len = dtzi
        .TimeZoneKeyName
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(128);
    String::from_utf16(&dtzi.TimeZoneKeyName[..len]).ok()
}

// ── §5  Storage ────────────────────────────────────────────────────────────────

/// Whether `localStorage` and `sessionStorage` are available.
#[derive(Clone, Debug)]
pub struct StorageInfo {
    pub local_storage: bool,
    pub session_storage: bool,
}

impl StorageInfo {
    pub fn detect() -> Self {
        // We plan to support both.
        Self {
            local_storage: true,
            session_storage: true,
        }
    }
}

// ── Top-level suite ────────────────────────────────────────────────────────────

/// All hard-to-access fingerprint values collected in one place.
#[derive(Clone, Debug)]
pub struct FingerprintSuite {
    pub canvas: CanvasFingerprint,
    pub webgl: WebGlInfo,
    pub audio: AudioFingerprint,
    pub fonts: FontList,
    pub touch: TouchInfo,
    pub overwrite: OverwriteInfo,
    pub nav_prototype: NavigatorPrototypeInfo,
    pub math: MathConstants,
    pub errors: ErrorShapeInfo,
    pub stack: StackDepthInfo,
    pub modernizr: ModernizrFlags,
    pub os_queries: OsMediaQueries,
    pub timezone: TimezoneInfo,
    pub storage: StorageInfo,
    pub unknown_image: UnknownImageInfo,
    pub adblock: bool,
}

impl FingerprintSuite {
    pub fn detect() -> Self {
        Self {
            canvas: CanvasFingerprint::detect(),
            webgl: WebGlInfo::detect(),
            audio: AudioFingerprint::detect(),
            fonts: FontList::detect(),
            touch: TouchInfo::detect(),
            overwrite: OverwriteInfo::detect(),
            nav_prototype: NavigatorPrototypeInfo::detect(),
            math: MathConstants::detect(),
            errors: ErrorShapeInfo::detect(),
            stack: StackDepthInfo::detect(),
            modernizr: ModernizrFlags::detect(),
            os_queries: OsMediaQueries::detect(),
            timezone: TimezoneInfo::detect(),
            storage: StorageInfo::detect(),
            unknown_image: UnknownImageInfo::detect(),
            adblock: false,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_constants_match_known_values() {
        let m = MathConstants::detect();
        assert!((m.asinh_1 - 0.881_373_587_019_543).abs() < 1e-12);
        assert!((m.atanh_half - 0.549_306_144_334_054_8).abs() < 1e-12);
        assert!((m.expm1_1 - 1.718_281_828_459_045).abs() < 1e-12);
        assert!((m.tanh_1 - 0.761_594_155_955_764_9).abs() < 1e-12);
        // acosh(1e300) is finite on modern libm; "Infinity" only on old Chromium bugs.
        assert!(!m.acosh_1e300.is_empty());
    }

    #[test]
    fn touch_string_format() {
        let t = TouchInfo::detect();
        assert_eq!(t.as_amiunique_string(), "0;false;false");
    }

    #[test]
    fn overwrite_all_native() {
        let o = OverwriteInfo::detect();
        assert!(o.all_native());
    }

    #[test]
    fn unknown_image_zero() {
        let u = UnknownImageInfo::detect();
        assert_eq!(u.as_amiunique_string(), "0;0");
    }

    #[test]
    fn os_queries_consistent_with_os() {
        let q = OsMediaQueries::detect();
        if std::env::consts::OS == "linux" {
            assert!(!q.mac && !q.win_xp && !q.win_vista && !q.win7 && !q.win8);
        }
    }

    #[test]
    fn timezone_offset_plausible() {
        let tz = TimezoneInfo::detect();
        assert!(
            tz.offset_minutes > -720 && tz.offset_minutes < 720,
            "offset {} out of [-720, 720]",
            tz.offset_minutes
        );
    }

    #[test]
    fn stack_depth_plausible() {
        let s = StackDepthInfo::detect();
        assert!(
            s.depth > 1_000 && s.depth < 100_000,
            "stack depth {} implausible",
            s.depth
        );
    }

    #[test]
    fn nav_prototype_non_empty() {
        let p = NavigatorPrototypeInfo::detect();
        assert!(!p.properties.is_empty());
        assert!(p.properties.contains(&"userAgent"));
    }

    #[test]
    fn canvas_data_url_is_placeholder() {
        let c = CanvasFingerprint::detect();
        assert!(c.data_url.starts_with("data:image/png;base64,ALMOSTTHERE"));
        assert!(c.is_native);
    }

    #[test]
    fn suite_constructs_without_panic() {
        let _ = FingerprintSuite::detect();
    }
}
