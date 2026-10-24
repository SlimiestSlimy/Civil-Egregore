//! The base game against the server: it reaches the server through
//! the simulation's API alone -- "basegame cant depend on server
//! internals, only the public apis". Utilities it may name: a shared
//! library, as protocol is ("yes in fine having basegame use utilities,
//! thats what utilities is for").

use std::fs;
use std::path::Path;

/// Every source under `folder`, its folders' too.
fn sources_in(folder: &Path, found_sources: &mut Vec<std::path::PathBuf>) {
    for folder_entry in fs::read_dir(folder).expect("a folder").flatten() {
        let entry_path = folder_entry.path();
        if entry_path.is_dir() {
            sources_in(&entry_path, found_sources);
        } else if entry_path.extension().is_some_and(|ending| ending == "rs") {
            found_sources.push(entry_path);
        }
    }
}

/// No line of the base game's sources names the server but by its
/// simulation's API. Comments are let be.
#[test]
fn the_base_game_asks_the_simulation_api_alone() {
    let root_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut source_files = Vec::new();
    sources_in(&root_folder.join("basegame/src"), &mut source_files);
    assert!(source_files.len() > 10, "{} sources found: the walk is broken", source_files.len());
    let mut wrong_places = Vec::new();
    for source_file in &source_files {
        let shown_path = source_file.strip_prefix(root_folder).expect("under the root").display().to_string();
        for (line_index, line) in fs::read_to_string(source_file).expect("read").lines().enumerate() {
            let code_part = line.split_once("//").map_or(line, |(before_comment, _)| before_comment);
            let past_the_api = code_part.replace("server::simulation::simulation_api::", "");
            if past_the_api.contains("server::") {
                wrong_places.push(format!("{shown_path}:{}: `{}`", line_index + 1, code_part.trim()));
            }
        }
    }
    assert!(wrong_places.is_empty(), "{} places where the base game reaches past the simulation's API:\n{}", wrong_places.len(), wrong_places.join("\n"));
}
