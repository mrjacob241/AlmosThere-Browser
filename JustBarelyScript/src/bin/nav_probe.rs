/// nav_probe — print inferred navigator.*, screen.*, and specs_placeholder values.
///
/// Usage:
///   cargo run --bin nav_probe
///   cargo run --bin nav_probe -- --json
use justbarelyscript::{FingerprintSuite, NavigatorInfo, ScreenInfo};

fn main() {
    let json_mode = std::env::args().any(|a| a == "--json");
    let nav = NavigatorInfo::detect();
    let scr = ScreenInfo::detect();
    let fp = FingerprintSuite::detect();

    if json_mode {
        print_json(&nav, &scr, &fp);
    } else {
        print_table(&nav, &scr, &fp);
    }
}

fn print_table(info: &NavigatorInfo, scr: &ScreenInfo, fp: &FingerprintSuite) {
    let nav_rows: &[(&str, String)] = &[
        ("platform", info.platform.clone()),
        ("userAgent", info.user_agent.clone()),
        ("appVersion", info.app_version.clone()),
        ("appName", info.app_name.into()),
        ("appCodeName", info.app_code_name.into()),
        ("product", info.product.into()),
        ("productSub", info.product_sub.into()),
        ("vendor", info.vendor.into()),
        ("vendorSub", format!("{:?}", info.vendor_sub)),
        ("hardwareConcurrency", info.hardware_concurrency.to_string()),
        ("maxTouchPoints", info.max_touch_points.to_string()),
        ("cookieEnabled", info.cookie_enabled.to_string()),
        (
            "doNotTrack",
            match info.do_not_track {
                Some(true) => "1".into(),
                Some(false) => "0".into(),
                None => "unspecified".into(),
            },
        ),
        (
            "language",
            info.languages.first().cloned().unwrap_or_default(),
        ),
        ("languages", info.languages.join(", ")),
        ("oscpu", opt_str(&info.oscpu)),
        ("cpuClass", opt_str(&info.cpu_class)),
        ("buildID", opt_str(&info.build_id)),
        ("(os_version)", info.os_version.clone()),
        ("(rust os)", std::env::consts::OS.into()),
        ("(rust arch)", std::env::consts::ARCH.into()),
    ];

    let scr_rows: &[(&str, String)] = &[
        ("width", scr.width.to_string()),
        ("height", scr.height.to_string()),
        ("colorDepth", scr.color_depth.to_string()),
        ("pixelDepth", scr.pixel_depth().to_string()),
        ("availWidth", scr.avail_width.to_string()),
        ("availHeight", scr.avail_height.to_string()),
    ];

    let tz = &fp.timezone;
    let gl = &fp.webgl;
    let fp_rows: &[(&str, String)] = &[
        // §4 Timezone
        ("timezoneOffset", tz.offset_minutes.to_string()),
        (
            "timezoneIANA",
            tz.iana_name.clone().unwrap_or_else(|| "(unknown)".into()),
        ),
        // §5 Storage
        ("localStorage", fp.storage.local_storage.to_string()),
        ("sessionStorage", fp.storage.session_storage.to_string()),
        // §7 WebGL
        (
            "webgl.vendor",
            if gl.is_supported {
                gl.vendor.clone()
            } else {
                "(none)".into()
            },
        ),
        (
            "webgl.renderer",
            if gl.is_supported {
                gl.renderer.clone()
            } else {
                "(none)".into()
            },
        ),
        // §8 Audio
        ("audio.supported", fp.audio.is_supported.to_string()),
        (
            "audio.pxi_sum",
            if fp.audio.is_supported {
                fp.audio.pxi_sum.to_string()
            } else {
                "(n/a)".into()
            },
        ),
        // §9 Fonts
        ("fonts.installed", fp.fonts.installed.len().to_string()),
        // §10 Touch
        ("touchSupport", fp.touch.as_amiunique_string()),
        // §11 Overwrite
        ("apis.native", fp.overwrite.all_native().to_string()),
        // §13 Math
        ("Math.asinh(1)", format!("{:.15}", fp.math.asinh_1)),
        ("Math.acosh(1e300)", fp.math.acosh_1e300.clone()),
        ("Math.atanh(0.5)", format!("{:.15}", fp.math.atanh_half)),
        ("Math.expm1(1)", format!("{:.15}", fp.math.expm1_1)),
        ("Math.tanh(1)", format!("{:.15}", fp.math.tanh_1)),
        // §15 Stack
        ("stackDepth", fp.stack.depth.to_string()),
        // §17 OS media queries
        ("osMediaQueries", fp.os_queries.as_amiunique_string()),
        // §18 AdBlock
        ("adBlock", fp.adblock.to_string()),
        // §19 Unknown image
        ("unknownImage", fp.unknown_image.as_amiunique_string()),
    ];

    // Compute column width across all three sections.
    let key_width = nav_rows
        .iter()
        .chain(scr_rows.iter())
        .chain(fp_rows.iter())
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(10);

    let sep = format!("  {}   {}", "─".repeat(key_width), "─".repeat(60));

    section(
        "navigator.* — inferred for this machine",
        nav_rows,
        key_width,
        &sep,
    );
    section(
        "screen.* — inferred for this machine",
        scr_rows,
        key_width,
        &sep,
    );
    section(
        "specs_placeholder — detected / mocked",
        fp_rows,
        key_width,
        &sep,
    );

    // Modernizr summary (separate because it's long).
    println!("\n  Modernizr flags\n");
    println!("{sep}");
    println!(
        "  {}",
        fp.modernizr.as_amiunique_string().replace(';', "\n  ")
    );
    println!();

    // Font list.
    if !fp.fonts.installed.is_empty() {
        println!("\n  Installed fonts ({} found)\n", fp.fonts.installed.len());
        println!("{sep}");
        for f in &fp.fonts.installed {
            println!("  {}", f);
        }
        println!();
    }
}

fn section(title: &str, rows: &[(&str, String)], key_width: usize, sep: &str) {
    println!("\n  {}\n", title);
    println!("  {:key_width$}   value", "property", key_width = key_width);
    println!("{sep}");
    for (key, val) in rows {
        println!("  {:key_width$}   {}", key, val, key_width = key_width);
    }
}

fn print_json(info: &NavigatorInfo, scr: &ScreenInfo, fp: &FingerprintSuite) {
    let gl = &fp.webgl;
    let pairs: Vec<String> = vec![
        // navigator.*
        jstr("platform", &info.platform),
        jstr("userAgent", &info.user_agent),
        jstr("appVersion", &info.app_version),
        jstr("appName", info.app_name),
        jstr("appCodeName", info.app_code_name),
        jstr("product", info.product),
        jstr("productSub", info.product_sub),
        jstr("vendor", info.vendor),
        jstr("vendorSub", info.vendor_sub),
        jnum("hardwareConcurrency", info.hardware_concurrency as f64),
        jnum("maxTouchPoints", info.max_touch_points as f64),
        jbool("cookieEnabled", info.cookie_enabled),
        jstr(
            "doNotTrack",
            match info.do_not_track {
                Some(true) => "1",
                Some(false) => "0",
                None => "unspecified",
            },
        ),
        jstr(
            "language",
            info.languages.first().map(String::as_str).unwrap_or(""),
        ),
        {
            let arr = info
                .languages
                .iter()
                .map(|l| format!("\"{}\"", l))
                .collect::<Vec<_>>()
                .join(", ");
            format!("  \"languages\": [{}]", arr)
        },
        jopt("oscpu", &info.oscpu),
        jopt("cpuClass", &info.cpu_class),
        jopt("buildID", &info.build_id),
        jstr("_os_version", &info.os_version),
        jstr("_rust_os", std::env::consts::OS),
        jstr("_rust_arch", std::env::consts::ARCH),
        // screen.*
        jnum("screen.width", scr.width as f64),
        jnum("screen.height", scr.height as f64),
        jnum("screen.colorDepth", scr.color_depth as f64),
        jnum("screen.pixelDepth", scr.pixel_depth() as f64),
        jnum("screen.availWidth", scr.avail_width as f64),
        jnum("screen.availHeight", scr.avail_height as f64),
        // specs_placeholder
        jnum("timezoneOffset", fp.timezone.offset_minutes as f64),
        jopt("timezoneIANA", &fp.timezone.iana_name),
        jbool("localStorage", fp.storage.local_storage),
        jbool("sessionStorage", fp.storage.session_storage),
        jstr(
            "webgl.vendor",
            if gl.is_supported { &gl.vendor } else { "" },
        ),
        jstr(
            "webgl.renderer",
            if gl.is_supported { &gl.renderer } else { "" },
        ),
        jbool("audio.supported", fp.audio.is_supported),
        jnum("audio.pxi_sum", fp.audio.pxi_sum),
        jnum("fonts.installed", fp.fonts.installed.len() as f64),
        jstr("touchSupport", &fp.touch.as_amiunique_string()),
        jbool("apis.native", fp.overwrite.all_native()),
        jnum("Math.asinh_1", fp.math.asinh_1),
        jstr("Math.acosh_1e300", &fp.math.acosh_1e300),
        jnum("Math.atanh_half", fp.math.atanh_half),
        jnum("Math.expm1_1", fp.math.expm1_1),
        jnum("Math.tanh_1", fp.math.tanh_1),
        jnum("stackDepth", fp.stack.depth as f64),
        jstr("osMediaQueries", &fp.os_queries.as_amiunique_string()),
        jbool("adBlock", fp.adblock),
        jstr("unknownImage", &fp.unknown_image.as_amiunique_string()),
    ];

    println!("{{\n{}\n}}", pairs.join(",\n"));
}

fn opt_str(o: &Option<String>) -> String {
    o.clone().unwrap_or_else(|| "(undefined)".into())
}

fn jstr(k: &str, v: &str) -> String {
    format!("  \"{}\": \"{}\"", k, v.replace('"', "\\\""))
}

fn jnum(k: &str, v: f64) -> String {
    format!("  \"{}\": {}", k, v)
}

fn jbool(k: &str, v: bool) -> String {
    format!("  \"{}\": {}", k, v)
}

fn jopt(k: &str, v: &Option<String>) -> String {
    match v {
        Some(s) => jstr(k, s),
        None => format!("  \"{}\": null", k),
    }
}
