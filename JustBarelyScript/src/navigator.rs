/// Inferred browser `navigator.*` properties, derived from OS-level APIs.
///
/// All fields mirror the names and semantics of their JS counterparts.
/// Detection is purely Rust-side (no browser, no JS); values represent what
/// a Chrome-like browser would expose on the current machine.
#[derive(Clone, Debug)]
pub struct NavigatorInfo {
    // ── Core identity ──────────────────────────────────────────────────────
    /// `navigator.platform`  e.g. "Win32", "MacIntel", "Linux x86_64"
    pub platform: String,
    /// `navigator.userAgent`
    pub user_agent: String,
    /// `navigator.appVersion`  (everything after "Mozilla/" in the UA)
    pub app_version: String,
    /// `navigator.appName`
    pub app_name: &'static str,
    /// `navigator.appCodeName`
    pub app_code_name: &'static str,
    /// `navigator.product`
    pub product: &'static str,
    /// `navigator.productSub`  — build-date stamp
    pub product_sub: &'static str,
    /// `navigator.vendor`
    pub vendor: &'static str,
    /// `navigator.vendorSub`  — always "" in every browser
    pub vendor_sub: &'static str,

    // ── Locale ─────────────────────────────────────────────────────────────
    /// `navigator.languages`  e.g. ["en-US", "en"]
    pub languages: Vec<String>,

    // ── Hardware ───────────────────────────────────────────────────────────
    /// `navigator.hardwareConcurrency`  logical CPU thread count
    pub hardware_concurrency: u32,
    /// `navigator.maxTouchPoints`  0 on non-touch desktop
    pub max_touch_points: u32,

    // ── Privacy / storage ──────────────────────────────────────────────────
    /// `navigator.cookieEnabled`
    pub cookie_enabled: bool,
    /// `navigator.doNotTrack`  None → "unspecified" / "NC"
    pub do_not_track: Option<bool>,

    // ── Browser-specific extras ────────────────────────────────────────────
    /// `navigator.oscpu`  Firefox-only  e.g. "Intel Mac OS X 10.15"
    pub oscpu: Option<String>,
    /// `navigator.cpuClass`  IE/Edge-only  e.g. "x86"
    pub cpu_class: Option<String>,
    /// `navigator.buildID`  Firefox-only  e.g. "20100101"
    pub build_id: Option<String>,

    // ── Raw OS info (not a JS property; used for UA building and tests) ────
    /// Detected OS version string as reported by the host OS
    pub os_version: String,
}

impl NavigatorInfo {
    /// Detect navigator properties from the current OS environment.
    pub fn detect() -> Self {
        let os_version = detect_os_version();
        let platform = detect_platform();
        let user_agent = build_user_agent(&os_version);
        let app_version = user_agent
            .strip_prefix("Mozilla/")
            .unwrap_or(&user_agent)
            .to_owned();
        let languages = detect_languages();
        let hardware_concurrency = detect_hardware_concurrency();
        let oscpu = detect_oscpu(&os_version);
        let cpu_class = detect_cpu_class();

        Self {
            platform,
            user_agent,
            app_version,
            app_name: "AlmosThere",
            app_code_name: "AlmostThere",
            product: "AlmosThere",
            product_sub: "20030107",
            vendor: "MrJacob241 AKA JohnHobbes",
            vendor_sub: "",
            languages,
            hardware_concurrency,
            max_touch_points: 0,
            cookie_enabled: true,
            do_not_track: None,
            oscpu,
            cpu_class,
            build_id: None,
            os_version,
        }
    }
}

// ── Platform string ────────────────────────────────────────────────────────────

fn detect_platform() -> String {
    match std::env::consts::OS {
        // Chrome on 64-bit Windows still reports "Win32" (historical quirk).
        // Firefox 64-bit reports "Win64".  We follow Chrome convention.
        "windows" => "Win32".to_owned(),
        // Both Intel and Apple Silicon Macs; Chrome reports "MacIntel" even on ARM.
        "macos" => "MacIntel".to_owned(),
        // Linux includes arch: "Linux x86_64", "Linux aarch64", etc.
        "linux" => format!("Linux {}", rust_arch_to_linux_arch()),
        // FreeBSD, NetBSD, Solaris, etc.
        other => format!("{} {}", capitalise(other), std::env::consts::ARCH),
    }
}

fn rust_arch_to_linux_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "x86" => "i686",
        "aarch64" => "aarch64",
        "arm" => "armv7l",
        "mips" => "mips",
        "mips64" => "mips64",
        "powerpc" => "ppc",
        "powerpc64" => "ppc64",
        "riscv64" => "riscv64",
        other => other,
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ── OS version detection ───────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn detect_os_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "10.15.7".to_owned())
}

#[cfg(target_os = "linux")]
fn detect_os_version() -> String {
    // /etc/os-release: VERSION_ID="22.04"
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("VERSION_ID="))
                .map(|l| {
                    l.trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_owned()
                })
        })
        .unwrap_or_else(|| {
            // Fallback: kernel version from uname -r
            std::process::Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_owned())
                .unwrap_or_else(|| "5.0".to_owned())
        })
}

#[cfg(target_os = "windows")]
fn detect_os_version() -> String {
    // "Microsoft Windows [Version 10.0.19041.572]\r\n"
    std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let start = s.find("Version ")? + "Version ".len();
            let rest = &s[start..];
            let end = rest.find(']').unwrap_or(rest.len());
            Some(rest[..end].trim().to_owned())
        })
        .unwrap_or_else(|| "10.0".to_owned())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn detect_os_version() -> String {
    "0.0".to_owned()
}

// ── User-Agent string ──────────────────────────────────────────────────────────

fn build_user_agent(_os_version: &str) -> String {
    "AlmostThere Browser/0.1.0".to_owned()
}

// ── Language detection ─────────────────────────────────────────────────────────

fn detect_languages() -> Vec<String> {
    // Linux/macOS: LANGUAGE env var holds a colon-separated ordered list.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Ok(list) = std::env::var("LANGUAGE") {
        let langs: Vec<String> = list
            .split(':')
            .filter(|s| !s.is_empty() && *s != "C")
            .map(locale_to_bcp47)
            .collect();
        if !langs.is_empty() {
            return dedup(langs);
        }
    }

    // LANG env var — present on Linux, macOS, and sometimes Windows (WSL).
    if let Ok(lang) = std::env::var("LANG") {
        let bcp = locale_to_bcp47(&lang);
        if bcp != "en-US" || lang.contains("en") {
            // Add the base language as a fallback.
            let base: String = bcp.split('-').next().unwrap_or("en").to_owned();
            return if base == bcp {
                vec![bcp]
            } else {
                vec![bcp, base]
            };
        }
    }

    // Windows: USERPROFILE doesn't help; use LC_ALL or fall back.
    #[cfg(target_os = "windows")]
    {
        if let Some(lang) = detect_windows_locale() {
            let base: String = lang.split('-').next().unwrap_or("en").to_owned();
            return if base == lang {
                vec![lang]
            } else {
                vec![lang, base]
            };
        }
    }

    // Safe default.
    vec!["en-US".to_owned(), "en".to_owned()]
}

/// Converts a POSIX locale string to a BCP-47 language tag.
/// "en_US.UTF-8" → "en-US",  "fr_FR" → "fr-FR",  "C" → "en-US"
fn locale_to_bcp47(locale: &str) -> String {
    let without_encoding = locale.split('.').next().unwrap_or(locale).trim();
    match without_encoding {
        "C" | "POSIX" | "" => "en-US".to_owned(),
        s => s.replace('_', "-"),
    }
}

/// Remove duplicates while preserving order.
fn dedup(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

#[cfg(target_os = "windows")]
fn detect_windows_locale() -> Option<String> {
    // PowerShell: (Get-Culture).Name  → "en-US"
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-Culture).Name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

// ── Hardware concurrency ───────────────────────────────────────────────────────

fn detect_hardware_concurrency() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
}

// ── Browser-specific extras ────────────────────────────────────────────────────

/// Firefox-only `navigator.oscpu` value.
fn detect_oscpu(os_version: &str) -> Option<String> {
    let s = match std::env::consts::OS {
        "macos" => format!("Intel Mac OS X {}", os_version),
        "windows" => format!("Windows NT {}", os_version),
        "linux" => format!("Linux {}", rust_arch_to_linux_arch()),
        _ => return None,
    };
    Some(s)
}

/// IE/Edge-only `navigator.cpuClass` — only meaningful on Windows.
fn detect_cpu_class() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // PROCESSOR_ARCHITECTURE: "AMD64", "x86", "ARM64"
        let arch = std::env::var("PROCESSOR_ARCHITECTURE")
            .unwrap_or_default()
            .to_lowercase();
        return Some(match arch.as_str() {
            "amd64" => "x86".to_owned(), // IE reported "x86" even on 64-bit
            "x86" => "x86".to_owned(),
            "arm64" => "ARM".to_owned(),
            other => other.to_owned(),
        });
    }
    #[cfg(not(target_os = "windows"))]
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_is_non_empty() {
        let info = NavigatorInfo::detect();
        assert!(!info.platform.is_empty(), "platform must be non-empty");
    }

    #[test]
    fn platform_matches_os() {
        let info = NavigatorInfo::detect();
        let os = std::env::consts::OS;
        if os == "windows" {
            assert!(
                info.platform.starts_with("Win"),
                "Windows platform should start with Win"
            );
        } else if os == "macos" {
            assert!(
                info.platform.starts_with("Mac"),
                "macOS platform should start with Mac"
            );
        } else if os == "linux" {
            assert!(
                info.platform.starts_with("Linux"),
                "Linux platform should start with Linux"
            );
        }
    }

    #[test]
    fn user_agent_is_almostthere() {
        let info = NavigatorInfo::detect();
        assert_eq!(info.user_agent, "AlmostThere Browser/0.1.0");
    }

    #[test]
    fn languages_non_empty() {
        let info = NavigatorInfo::detect();
        assert!(
            !info.languages.is_empty(),
            "languages must have at least one entry"
        );
    }

    #[test]
    fn languages_are_bcp47_shaped() {
        let info = NavigatorInfo::detect();
        for lang in &info.languages {
            // BCP-47 tags use hyphens, not underscores; no encoding suffix
            assert!(
                !lang.contains('_'),
                "language '{}' must not contain underscore",
                lang
            );
            assert!(
                !lang.contains('.'),
                "language '{}' must not contain dot",
                lang
            );
            assert!(!lang.is_empty(), "language entry must not be empty");
        }
    }

    #[test]
    fn hardware_concurrency_at_least_one() {
        let info = NavigatorInfo::detect();
        assert!(info.hardware_concurrency >= 1);
    }

    #[test]
    fn constant_fields_have_correct_values() {
        let info = NavigatorInfo::detect();
        assert_eq!(info.app_name, "AlmosThere");
        assert_eq!(info.app_code_name, "AlmostThere");
        assert_eq!(info.product, "AlmosThere");
        assert_eq!(info.vendor, "MrJacob241 AKA JohnHobbes");
        assert_eq!(info.vendor_sub, "");
    }

    #[test]
    fn os_version_non_empty() {
        let info = NavigatorInfo::detect();
        assert!(!info.os_version.is_empty(), "os_version must be non-empty");
    }

    #[test]
    fn oscpu_present_on_known_os() {
        let info = NavigatorInfo::detect();
        let os = std::env::consts::OS;
        if matches!(os, "macos" | "linux" | "windows") {
            assert!(info.oscpu.is_some(), "oscpu should be Some on {}", os);
        }
    }

    #[test]
    fn locale_to_bcp47_conversions() {
        assert_eq!(locale_to_bcp47("en_US.UTF-8"), "en-US");
        assert_eq!(locale_to_bcp47("fr_FR"), "fr-FR");
        assert_eq!(locale_to_bcp47("de_DE.UTF-8"), "de-DE");
        assert_eq!(locale_to_bcp47("C"), "en-US");
        assert_eq!(locale_to_bcp47("POSIX"), "en-US");
        assert_eq!(locale_to_bcp47("zh_CN.UTF-8"), "zh-CN");
    }

    #[test]
    fn platform_linux_includes_arch() {
        // Only meaningful to run on Linux; on other OS this is a smoke test.
        if std::env::consts::OS == "linux" {
            let info = NavigatorInfo::detect();
            assert!(
                info.platform.contains(rust_arch_to_linux_arch()),
                "Linux platform '{}' must include arch '{}'",
                info.platform,
                rust_arch_to_linux_arch()
            );
        }
    }
}
