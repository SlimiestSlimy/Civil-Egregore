//! PNG images of the bitmaps looked at, written to
//! `transient_data/renders/`.

use crate::diagnostics::bitmaps::looked_at;
use crate::diagnostics::png::bitmap_png;
use std::fs;
use utilities::diagnostics::table::Table;
use std::path::Path;
use crate::transient_data;

/// Writes a PNG of every bitmap looked at, and prints a table of them:
/// each bitmap, its cells set, and where its image went.
pub fn run_render_tool() {
    let folder = transient_data::renders_folder();
    fs::create_dir_all(&folder).expect("the images' folder made");
    // Where each image went, from the crate's folder.
    let shown_folder = folder.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&folder).to_path_buf();
    let mut table = Table::new(&["bitmap", "cells\nset", "image"]).left_aligned(&["image"]);
    for (bitmap_name, bitmap) in looked_at() {
        let image_file = format!("{bitmap_name}.png");
        fs::write(folder.join(&image_file), bitmap_png(&bitmap)).expect("the image written");
        table.add_row(&[bitmap_name, bitmap.count_set().to_string(), Path::new(&shown_folder).join(&image_file).display().to_string()]);
    }
    table.print_rendered();
}
