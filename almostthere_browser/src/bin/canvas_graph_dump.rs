use std::{env, fs, io, path::Path};

mod browser_app {
    #![allow(dead_code)]

    include!("../main.rs");

    pub fn load_for_capture(input: &str) -> std::io::Result<rich_canvas::BrowserDocument> {
        load_url_document(input)
    }
}

const OUTPUT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/url_screenshots");

fn main() -> io::Result<()> {
    let url = env::args()
        .nth(1)
        .unwrap_or_else(|| "https://latex.vercel.app/elements".to_owned());
    let output_name = env::args()
        .nth(2)
        .unwrap_or_else(|| "canvas_graph_dump.txt".to_owned());

    fs::create_dir_all(OUTPUT_DIR)?;

    let document = browser_app::load_for_capture(&url)?;
    let dump = format!("{:#?}\n", document.canvas_graph);
    let output_path = Path::new(OUTPUT_DIR).join(output_name);
    fs::write(&output_path, &dump)?;

    println!("url: {url}");
    println!("canvas_graph_dump: {}", output_path.display());
    println!("objects: {}", document.canvas_graph.objects.len());
    println!("bytes: {}", dump.len());

    Ok(())
}
