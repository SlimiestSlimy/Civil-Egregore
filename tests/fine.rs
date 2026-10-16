//! The fine tier of the workspace's own tests: the docs against the
//! code. Every markdown file's links lead somewhere, and every path and
//! every name it writes in backticks is one the repository has -- so a
//! doc left behind by the code fails here, not in a reader's hands.
//! What counts as a path and a name, and what is let be:
//! `docs/testing_protocol.md`, "The docs are tested too".
//!
//! `cargo test --test fine`

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Folders not looked in: what is built, what runs leave, and what is
/// not the repository's own.
const NOT_LOOKED_IN: [&str; 4] = ["target", ".git", "transient_data", "delete_after_use"];

/// What a file a run writes ends in: one named after the tool that
/// writes it is there if the code has the tool's name.
const WRITTEN_BY_RUNS: [&str; 3] = [".csv", ".png", ".pbm"];

/// What a path ends in, if it has no `/` to say it is one.
const FILE_ENDINGS: [&str; 8] = [".rs", ".md", ".csv", ".toml", ".txt", ".png", ".pbm", ".json"];

/// Names the code does not have and the docs may still write: Rust's
/// own and cargo's, said in passing.
const NAMES_NOT_OURS: [&str; 14] = ["Vec", "Option", "Arc", "HashMap", "BTreeMap", "Instant", "AddAssign", "Default", "PhantomData", "String", "Ord", "CARGO_MANIFEST_DIR", "RUSTFLAGS", "SystemTime"];

/// Every file under `folder` whose name ends in one of `wanted_endings`, but
/// for the folders [`NOT_LOOKED_IN`].
fn collect_files(folder: &Path, wanted_endings: &[&str], found_files: &mut Vec<PathBuf>) {
    let Ok(folder_entries) = fs::read_dir(folder) else {
        return;
    };
    for folder_entry in folder_entries.flatten() {
        let (path, entry_name) = (folder_entry.path(), folder_entry.file_name().to_string_lossy().into_owned());
        if path.is_dir() {
            if !NOT_LOOKED_IN.contains(&entry_name.as_str()) {
                collect_files(&path, wanted_endings, found_files);
            }
        } else if wanted_endings.iter().any(|wanted_ending| entry_name.ends_with(wanted_ending)) {
            found_files.push(path);
        }
    }
}

/// Whether `candidate_text` is a name as code writes one: letters, digits and
/// `_`, not starting with a digit.
fn is_a_name(candidate_text: &str) -> bool {
    !candidate_text.is_empty() && !candidate_text.starts_with(|first_char: char| first_char.is_ascii_digit()) && candidate_text.chars().all(|name_char| name_char.is_ascii_alphanumeric() || name_char == '_')
}

/// Every name in the repository's code and settings: each run of
/// letters, digits and `_` in a source, a manifest or a settings file,
/// and every file's and folder's own name, with and without its ending.
fn names_in_the_code(root_folder: &Path) -> HashSet<String> {
    let mut source_files = Vec::new();
    collect_files(root_folder, &[".rs", ".toml", ".csv"], &mut source_files);
    let mut code_names = HashSet::new();
    for source_file in &source_files {
        let source_text = fs::read_to_string(source_file).unwrap_or_default();
        code_names.extend(source_text.split(|source_char: char| !(source_char.is_ascii_alphanumeric() || source_char == '_')).filter(|word| !word.is_empty()).map(str::to_string));
        for part in source_file.strip_prefix(root_folder).expect("under the root").components() {
            let part = part.as_os_str().to_string_lossy();
            code_names.insert(part.split('.').next().unwrap_or_default().to_string());
        }
    }
    code_names
}

/// The lines of `markdown_text` outside its fenced blocks, each with its number.
fn prose_lines(markdown_text: &str) -> Vec<(usize, &str)> {
    let mut in_fence = false;
    markdown_text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let is_fence_line = line.trim_start().starts_with("```");
            in_fence ^= is_fence_line;
            !in_fence && !is_fence_line
        })
        .map(|(line_index, line)| (line_index + 1, line))
        .collect()
}

/// What `line` writes in backticks, a span each.
fn backtick_spans(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// Where `line`'s links lead: what is in the brackets after each `](`.
fn link_targets(line: &str) -> Vec<&str> {
    line.split("](").skip(1).filter_map(|after_bracket| after_bracket.split(')').next()).collect()
}

/// The folder of the crate `file_path` is in: the nearest above it with a
/// manifest.
fn crate_of(file_path: &Path) -> &Path {
    file_path.ancestors().skip(1).find(|folder| folder.join("Cargo.toml").exists()).expect("in the workspace")
}

/// What the repository is searched by, gathered once: every name its
/// code has, every file's and folder's own name, every crate's folder,
/// and the code's whole text.
struct RepositoryContents {
    /// The workspace's root.
    root_folder: PathBuf,
    /// Every name in the code ([`names_in_the_code`]), and every
    /// file's name without its ending.
    code_names: HashSet<String>,
    /// Every file and folder there is.
    all_paths: Vec<PathBuf>,
    /// Every folder with a manifest: the crates, the root among them.
    crate_folders: Vec<PathBuf>,
    /// The sources, manifests and settings, one after another: what a
    /// file a run writes is named in.
    code_text: String,
}

impl RepositoryContents {
    /// The repository at `root_folder`, read whole.
    fn read_whole(root_folder: &Path) -> RepositoryContents {
        let mut all_files = Vec::new();
        collect_files(root_folder, &[""], &mut all_files);
        let folders: HashSet<PathBuf> = all_files.iter().flat_map(|listed_file| listed_file.ancestors().skip(1).take_while(|folder| *folder != root_folder)).map(Path::to_path_buf).collect();
        let all_paths = all_files.iter().cloned().chain(folders).collect();
        let crate_folders = all_files.iter().filter(|possible_manifest| possible_manifest.ends_with("Cargo.toml")).map(|manifest_path| manifest_path.parent().expect("in a folder").to_path_buf()).collect();
        let code_text = all_files.iter().filter(|code_file| [".rs", ".toml", ".csv"].iter().any(|code_ending| code_file.to_string_lossy().ends_with(code_ending))).map(|text_file| fs::read_to_string(text_file).unwrap_or_default()).collect();
        let mut code_names = names_in_the_code(root_folder);
        code_names.extend(all_files.iter().filter_map(|named_file| named_file.file_stem()).map(|stem_name| stem_name.to_string_lossy().into_owned()));
        RepositoryContents { root_folder: root_folder.to_path_buf(), code_names, all_paths, crate_folders, code_text }
    }

    /// Whether `path`, as `writing_file` writes it, is there: found from where
    /// it may be meant, or a bare name a file of the crate has or a run
    /// writes.
    fn has_path(&self, writing_file: &Path, path: &str) -> bool {
        let path = path.trim_end_matches('/');
        if path.contains('/') {
            return self.resolve_path(writing_file, path).is_some();
        }
        let named_by_its_tool = WRITTEN_BY_RUNS.iter().any(|run_ending| path.strip_suffix(run_ending).is_some_and(|tool_name| self.code_names.contains(tool_name)));
        self.files_about(writing_file).iter().any(|own_path| own_path.file_name().is_some_and(|own_file_name| own_file_name == path)) || self.code_text.contains(&format!("\"{path}\"")) || named_by_its_tool
    }

    /// The files and folders a bare name in `writing_file` may be of: its own
    /// crate's -- every crate's for a doc of the workspace's, which
    /// speaks of them all.
    fn files_about(&self, writing_file: &Path) -> Vec<&PathBuf> {
        let own_crate = crate_of(writing_file);
        self.all_paths.iter().filter(|listed_path| own_crate == self.root_folder || listed_path.starts_with(own_crate)).collect()
    }

    /// Where `path`, as `writing_file` writes it, is: the first place it may
    /// be meant from that has it.
    fn resolve_path(&self, writing_file: &Path, path: &str) -> Option<PathBuf> {
        let own_crate = crate_of(writing_file);
        let crates_meant = self.crate_folders.iter().map(PathBuf::as_path).filter(|crate_folder| own_crate == self.root_folder || *crate_folder == own_crate);
        let mut search_folders = vec![writing_file.parent().expect("in a folder").to_path_buf(), self.root_folder.clone()];
        search_folders.extend(crates_meant.flat_map(|folder| [folder.to_path_buf(), folder.join("src"), folder.join("tests")]));
        search_folders.iter().map(|search_folder| search_folder.join(path)).find(|candidate_path| candidate_path.exists())
    }

    /// The sections `writing_file` is told to have that it has not: the titles
    /// quoted in `text_after_path`, what follows the path.
    fn sections_missing(&self, writing_file: &Path, path: &str, text_after_path: &str) -> Vec<String> {
        let Some(doc_text) = self.resolve_path(writing_file, path).and_then(|found_doc| fs::read_to_string(found_doc).ok()) else {
            return Vec::new();
        };
        let doc_headings: Vec<&str> = doc_text.lines().filter(|line| line.starts_with('#') || line.starts_with("**")).map(|line| line.trim_start_matches(['#', '*', '`', ' ']).trim()).collect();
        let collapsed_after = text_after_path.split_whitespace().collect::<Vec<_>>().join(" ");
        let (mut missing_titles, mut parts) = (Vec::new(), collapsed_after.split('"'));
        // A title's own backticks are left off; the titles end with the sentence, a bracket, or other code.
        while let (Some(text_between), Some(quoted_title)) = (parts.next(), parts.next()) {
            if text_between.contains(['`', '|', ')']) || text_between.contains(". ") {
                break;
            }
            let bare_title = quoted_title.trim_matches('`');
            if !doc_headings.iter().any(|doc_heading| doc_heading.starts_with(bare_title)) {
                missing_titles.push(bare_title.to_string());
            }
        }
        missing_titles
    }
}

/// Whether `backtick_span` is written as a path: no space in it, a `/` or a
/// file's ending, and nothing that makes it a pattern, a command or a
/// place outside the repository.
fn is_a_path(backtick_span: &str) -> bool {
    let plain = !backtick_span.contains(|span_char: char| span_char.is_whitespace() || "<>*{}$~=:(,".contains(span_char)) && !backtick_span.starts_with('-') && !backtick_span.starts_with('/');
    let not_excluded = !NOT_LOOKED_IN.iter().any(|folder| backtick_span.split('/').any(|part| part == *folder));
    plain && not_excluded && (backtick_span.contains('/') || FILE_ENDINGS.iter().any(|file_ending| backtick_span.ends_with(file_ending))) && backtick_span.chars().any(|span_letter| span_letter.is_ascii_alphabetic())
}

/// The names `backtick_span` writes, if it is written as code: a path of names
/// joined by `::` or `.`, with or without what it is called with --
/// and looking like code, not a word: a `::`, a `_`, a call, or a
/// capital in it.
fn names_of(backtick_span: &str) -> Option<Vec<&str>> {
    let called_path = backtick_span.split('(').next().unwrap_or_default().split('<').next().unwrap_or_default().trim_start_matches('&');
    let like_code = called_path.contains("::") || called_path.contains('_') || backtick_span.contains('(') || called_path.chars().any(|called_char| called_char.is_ascii_uppercase());
    let parts: Vec<&str> = called_path.split("::").flat_map(|part| part.split('.')).collect();
    (like_code && parts.iter().all(|part| is_a_name(part))).then_some(parts)
}

/// Every markdown file of the repository: its links lead to files
/// that are there, the paths it writes in backticks are there, and
/// the names it writes in backticks are names the code has.
#[test]
fn the_docs_name_what_is_there() {
    let root_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_contents = RepositoryContents::read_whole(root_folder);
    let mut markdown_files = Vec::new();
    collect_files(root_folder, &[".md"], &mut markdown_files);
    assert!(markdown_files.len() > 30, "{} docs found: the walk is broken", markdown_files.len());
    let mut wrong_places = Vec::new();
    for markdown_file in &markdown_files {
        let doc_text = fs::read_to_string(markdown_file).expect("read");
        let shown_path = markdown_file.strip_prefix(root_folder).expect("under the root").display().to_string();
        let lines = prose_lines(&doc_text);
        for (place, &(line_number, line)) in lines.iter().enumerate() {
            for link_text in link_targets(line) {
                let link_target = link_text.split('#').next().unwrap_or_default();
                if !link_target.is_empty() && !link_target.contains("://") && !link_target.starts_with("mailto:") && !markdown_file.parent().expect("in a folder").join(link_target).exists() {
                    wrong_places.push(format!("{shown_path}:{line_number}: the link to `{link_text}` leads nowhere"));
                }
            }
            for backtick_span in backtick_spans(line) {
                if is_a_path(backtick_span) {
                    if !repository_contents.has_path(markdown_file, backtick_span) {
                        wrong_places.push(format!("{shown_path}:{line_number}: no file or folder `{backtick_span}`"));
                    } else if backtick_span.ends_with(".md") && backtick_span.contains('/') {
                        let text_after_span = line.split_once(&format!("`{backtick_span}`")).map_or("", |(_, split_after)| split_after);
                        let next_line = lines.get(place + 1).map_or("", |&(_, following_line)| following_line);
                        for missing_title in repository_contents.sections_missing(markdown_file, backtick_span, &format!("{text_after_span} {next_line}")) {
                            wrong_places.push(format!("{shown_path}:{line_number}: `{backtick_span}` has no section \"{missing_title}\""));
                        }
                    }
                } else if let Some(parts) = names_of(backtick_span) {
                    for part in parts.into_iter().filter(|part| !repository_contents.code_names.contains(*part) && !NAMES_NOT_OURS.contains(part)) {
                        wrong_places.push(format!("{shown_path}:{line_number}: `{backtick_span}`: the code has no `{part}`"));
                    }
                }
            }
        }
    }
    assert!(wrong_places.is_empty(), "{} places where the docs name what is not there:\n{}", wrong_places.len(), wrong_places.join("\n"));
}

/// The names of the items `source_text` has, private ones too: what
/// follows the kind of item, a name each. Not what a trait makes a
/// type have -- an `impl ... for` block's -- nor the tests at its end.
fn item_names(source_text: &str) -> Vec<&str> {
    const ITEM_KEYWORDS: [&str; 8] = ["fn", "struct", "enum", "trait", "type", "const", "static", "mod"];
    let (mut found_names, mut within_a_traits) = (Vec::new(), None);
    for line in source_text.lines() {
        let indent_width = line.len() - line.trim_start().len();
        let line = line.trim_start();
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        // A trait's block for a type runs to the brace closing it, as far in as it began.
        if let Some(began_indent) = within_a_traits {
            within_a_traits = (indent_width != began_indent || !line.starts_with('}')).then_some(began_indent);
            continue;
        }
        if line.starts_with("impl") && line.contains(" for ") && line.ends_with('{') {
            within_a_traits = Some(indent_width);
            continue;
        }
        let shown_to = line.strip_prefix("pub").map_or(line, |after_pub| after_pub.trim_start_matches(|prefix_char: char| prefix_char != ' ').trim_start());
        let mut words: Vec<&str> = shown_to.split_whitespace().collect();
        while words.len() > 2 && ["const", "unsafe"].contains(&words[0]) && words[1] == "fn" || words.first() == Some(&"unsafe") {
            words.remove(0);
        }
        if let [item_keyword, item_name, ..] = words[..]
            && ITEM_KEYWORDS.contains(&item_keyword)
        {
            found_names.push(item_name.split(|name_char: char| !(name_char.is_ascii_alphanumeric() || name_char == '_')).next().unwrap_or_default());
        }
    }
    found_names.retain(|kept_name| !kept_name.is_empty() && *kept_name != "_");
    found_names
}

/// Every crate with docs names in them every item it has, private
/// ones too: a reference goes function by function, and none is left
/// out.
#[test]
fn every_item_is_in_its_crates_docs() {
    let root_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_contents = RepositoryContents::read_whole(root_folder);
    let mut missing_items = Vec::new();
    for crate_folder in repository_contents.crate_folders.iter().filter(|candidate_crate| candidate_crate.join("docs").is_dir() && *candidate_crate != root_folder) {
        let (mut crate_docs, mut crate_sources) = (Vec::new(), Vec::new());
        collect_files(crate_folder, &[".md"], &mut crate_docs);
        collect_files(&crate_folder.join("src"), &[".rs"], &mut crate_sources);
        let docs_text: String = crate_docs.iter().filter(|crate_doc| crate_of(crate_doc) == crate_folder).map(|read_doc| fs::read_to_string(read_doc).unwrap_or_default()).collect();
        let said_words: HashSet<&str> = docs_text.split(|said_char: char| !(said_char.is_ascii_alphanumeric() || said_char == '_')).collect();
        for source_file in &crate_sources {
            let source_text = fs::read_to_string(source_file).expect("read");
            let shown_path = source_file.strip_prefix(root_folder).expect("under the root").display().to_string();
            missing_items.extend(item_names(&source_text).into_iter().filter(|unsaid_name| !said_words.contains(unsaid_name)).map(|missing_name| format!("{shown_path}: `{missing_name}`")));
        }
    }
    assert!(missing_items.is_empty(), "{} items no doc of their crate names:\n{}", missing_items.len(), missing_items.join("\n"));
}

/// Every doc a comment in the code points at is there, and has the
/// section the comment names.
#[test]
fn the_code_points_at_docs_that_are_there() {
    let root_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_contents = RepositoryContents::read_whole(root_folder);
    let mut source_files = Vec::new();
    collect_files(root_folder, &[".rs"], &mut source_files);
    let mut wrong_places = Vec::new();
    for source_file in &source_files {
        let source_text = fs::read_to_string(source_file).expect("read");
        let shown_path = source_file.strip_prefix(root_folder).expect("under the root").display().to_string();
        let comment_lines: Vec<(usize, &str)> = source_text.lines().enumerate().filter_map(|(line_index, line)| line.split_once("//").map(|(_, comment_text)| (line_index + 1, comment_text.trim_start_matches(['/', '!'])))).collect();
        for (place, &(line_number, line_comment)) in comment_lines.iter().enumerate() {
            for doc_span in backtick_spans(line_comment).into_iter().filter(|candidate_span| candidate_span.ends_with(".md") && is_a_path(candidate_span)) {
                let from_the_crate = doc_span.trim_start_matches("../");
                let Some(pointed_doc) = [crate_of(source_file), root_folder].iter().map(|start_folder| start_folder.join(from_the_crate)).find(|candidate_path| candidate_path.exists()) else {
                    wrong_places.push(format!("{shown_path}:{line_number}: no doc `{doc_span}`"));
                    continue;
                };
                let text_after_span = line_comment.split_once(&format!("`{doc_span}`")).map_or("", |(_, split_after)| split_after);
                let next_comment = comment_lines.get(place + 1).filter(|(following_number, _)| *following_number == line_number + 1).map_or("", |&(_, following_comment)| following_comment);
                let path_from_the_root = pointed_doc.strip_prefix(root_folder).expect("under the root").to_string_lossy().into_owned();
                for missing_title in repository_contents.sections_missing(&root_folder.join("README.md"), &path_from_the_root, &format!("{text_after_span} {next_comment}")) {
                    wrong_places.push(format!("{shown_path}:{line_number}: `{doc_span}` has no section \"{missing_title}\""));
                }
            }
        }
    }
    assert!(wrong_places.is_empty(), "{} comments pointing at docs that are not there:\n{}", wrong_places.len(), wrong_places.join("\n"));
}

/// The crates rules are written in: what they depend on is
/// `actions` alone, and they ask their turn nothing themselves.
const RULES_CRATES: [&str; 2] = ["sca_rules", "entity_rules"];

/// A rule asks actions, and nothing else: no rule's source calls
/// anything of its turn's own -- the turn is handed on to an
/// action, never asked -- nor names a crate under the
/// actions (`actions/docs/actions.md`, "Rules ask
/// actions, and nothing else").
#[test]
fn the_rules_ask_actions_alone() {
    let root_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_under_actions = ["compares_and_writes::", "simulation::", "bitplane_manager::", "entity_manager::", "chunk_storage::", "coordinates::", "type_registry::", "utilities::", "pathfinding::", "worldgen::", "server::"];
    let mut wrong_places = Vec::new();
    for rules in RULES_CRATES {
        let mut source_files = Vec::new();
        collect_files(&root_folder.join(rules).join("src"), &[".rs"], &mut source_files);
        assert!(!source_files.is_empty(), "{rules} has no sources");
        for source_file in &source_files {
            let source_text = fs::read_to_string(source_file).expect("read");
            let shown_path = source_file.strip_prefix(root_folder).expect("under the root").display().to_string();
            for (line_index, line) in source_text.lines().enumerate() {
                // What is said in a comment is not asked.
                let code_part = line.split_once("//").map_or(line, |(before_comment, _)| before_comment);
                if code_part.contains("turn.") {
                    wrong_places.push(format!("{shown_path}:{}: asks its turn itself: `{}`", line_index + 1, code_part.trim()));
                }
                if let Some(named_crate) = crates_under_actions.iter().find(|crate_prefix| code_part.contains(**crate_prefix) && !code_part.contains(&format!("actions::{crate_prefix}"))) {
                    wrong_places.push(format!("{shown_path}:{}: names `{}`, under the actions", line_index + 1, named_crate.trim_end_matches(':')));
                }
            }
        }
    }
    assert!(wrong_places.is_empty(), "{} places where a rule reaches past the actions:\n{}", wrong_places.len(), wrong_places.join("\n"));
}
