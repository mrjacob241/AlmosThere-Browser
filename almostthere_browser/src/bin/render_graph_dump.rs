use std::{env, fs, io, path::Path};

mod browser_app {
    #![allow(dead_code)]

    include!("../main.rs");

    pub fn render_graph_dump_for_capture(input: &str) -> std::io::Result<String> {
        load_render_graph_debug_dump(input)
    }
}

const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/url_screenshots");

fn main() -> io::Result<()> {
    let url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://latex.vercel.app/elements".to_owned());
    let output_name = env::args()
        .nth(2)
        .unwrap_or_else(|| "render_graph_dump.txt".to_owned());

    fs::create_dir_all(OUTPUT_DIR)?;

    let dump = browser_app::render_graph_dump_for_capture(&url)?;
    let output_path = Path::new(OUTPUT_DIR).join(output_name);
    fs::write(&output_path, &dump)?;

    println!("url: {url}");
    println!("render_graph_dump: {}", output_path.display());
    println!("lines: {}", dump.lines().count());
    println!("bytes: {}", dump.len());
    println!("first_lines:");
    for line in dump.lines().take(80) {
        println!("{line}");
    }

    Ok(())
}
