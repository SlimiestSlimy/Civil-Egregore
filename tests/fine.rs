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

/// Every file under `folder` whose name ends in one of `endings`, but
/// for the folders [`NOT_LOOKED_IN`].
fn files(folder: &Path, endings: &[&str], found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let (path, name) = (entry.path(), entry.file_name().to_string_lossy().into_owned());
        if path.is_dir() {
            if !NOT_LOOKED_IN.contains(&name.as_str()) {
                files(&path, endings, found);
            }
        } else if endings.iter().any(|ending| name.ends_with(ending)) {
            found.push(path);
        }
    }
}

/// Whether `text` is a name as code writes one: letters, digits and
/// `_`, not starting with a digit.
fn is_a_name(text: &str) -> bool {
    !text.is_empty() && !text.starts_with(|first: char| first.is_ascii_digit()) && text.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
}

/// Every name in the repository's code and settings: each run of
/// letters, digits and `_` in a source, a manifest or a settings file,
/// and every file's and folder's own name, with and without its ending.
fn names_in_the_code(root: &Path) -> HashSet<String> {
    let mut sources = Vec::new();
    files(root, &[".rs", ".toml", ".csv"], &mut sources);
    let mut names = HashSet::new();
    for source in &sources {
        let text = fs::read_to_string(source).unwrap_or_default();
        names.extend(text.split(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_')).filter(|word| !word.is_empty()).map(str::to_string));
        for part in source.strip_prefix(root).expect("under the root").components() {
            let part = part.as_os_str().to_string_lossy();
            names.insert(part.split('.').next().unwrap_or_default().to_string());
        }
    }
    names
}

/// The lines of `text` outside its fenced blocks, each with its number.
fn prose(text: &str) -> Vec<(usize, &str)> {
    let mut fenced = false;
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let fence = line.trim_start().starts_with("```");
            fenced ^= fence;
            !fenced && !fence
        })
        .map(|(number, line)| (number + 1, line))
        .collect()
}

/// What `line` writes in backticks, a span each.
fn spans(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// Where `line`'s links lead: what is in the brackets after each `](`.
fn links(line: &str) -> Vec<&str> {
    line.split("](").skip(1).filter_map(|after| after.split(')').next()).collect()
}

/// The folder of the crate `file` is in: the nearest above it with a
/// manifest.
fn crate_of(file: &Path) -> &Path {
    file.ancestors().skip(1).find(|folder| folder.join("Cargo.toml").exists()).expect("in the workspace")
}

/// What the repository is searched by, gathered once: every name its
/// code has, every file's and folder's own name, every crate's folder,
/// and the code's whole text.
struct Repository {
    /// The workspace's root.
    root: PathBuf,
    /// Every name in the code ([`names_in_the_code`]), and every
    /// file's name without its ending.
    names: HashSet<String>,
    /// Every file and folder there is.
    everything: Vec<PathBuf>,
    /// Every folder with a manifest: the crates, the root among them.
    crates: Vec<PathBuf>,
    /// The sources, manifests and settings, one after another: what a
    /// file a run writes is named in.
    text: String,
}

impl Repository {
    /// The repository at `root`, read whole.
    fn read(root: &Path) -> Repository {
        let mut sources = Vec::new();
        files(root, &[""], &mut sources);
        let folders: HashSet<PathBuf> = sources.iter().flat_map(|file| file.ancestors().skip(1).take_while(|folder| *folder != root)).map(Path::to_path_buf).collect();
        let everything = sources.iter().cloned().chain(folders).collect();
        let crates = sources.iter().filter(|file| file.ends_with("Cargo.toml")).map(|manifest| manifest.parent().expect("in a folder").to_path_buf()).collect();
        let text = sources.iter().filter(|file| [".rs", ".toml", ".csv"].iter().any(|ending| file.to_string_lossy().ends_with(ending))).map(|file| fs::read_to_string(file).unwrap_or_default()).collect();
        let mut names = names_in_the_code(root);
        names.extend(sources.iter().filter_map(|file| file.file_stem()).map(|stem| stem.to_string_lossy().into_owned()));
        Repository { root: root.to_path_buf(), names, everything, crates, text }
    }

    /// Whether `path`, as `file` writes it, is there: found from where
    /// it may be meant, or a bare name a file of the crate has or a run
    /// writes.
    fn has_path(&self, file: &Path, path: &str) -> bool {
        let path = path.trim_end_matches('/');
        if path.contains('/') {
            return self.found(file, path).is_some();
        }
        let named_by_its_tool = WRITTEN_BY_RUNS.iter().any(|ending| path.strip_suffix(ending).is_some_and(|tool| self.names.contains(tool)));
        self.files_about(file).iter().any(|own| own.file_name().is_some_and(|name| name == path)) || self.text.contains(&format!("\"{path}\"")) || named_by_its_tool
    }

    /// The files and folders a bare name in `file` may be of: its own
    /// crate's -- every crate's for a doc of the workspace's, which
    /// speaks of them all.
    fn files_about(&self, file: &Path) -> Vec<&PathBuf> {
        let own = crate_of(file);
        self.everything.iter().filter(|there| own == self.root || there.starts_with(own)).collect()
    }

    /// Where `path`, as `file` writes it, is: the first place it may
    /// be meant from that has it.
    fn found(&self, file: &Path, path: &str) -> Option<PathBuf> {
        let own = crate_of(file);
        let crates = self.crates.iter().map(PathBuf::as_path).filter(|other| own == self.root || *other == own);
        let mut from = vec![file.parent().expect("in a folder").to_path_buf(), self.root.clone()];
        from.extend(crates.flat_map(|folder| [folder.to_path_buf(), folder.join("src"), folder.join("tests")]));
        from.iter().map(|from| from.join(path)).find(|there| there.exists())
    }

    /// The sections `file` is told to have that it has not: the titles
    /// quoted in `after`, what follows the path.
    fn sections_missing(&self, file: &Path, path: &str, after: &str) -> Vec<String> {
        let Some(text) = self.found(file, path).and_then(|there| fs::read_to_string(there).ok()) else {
            return Vec::new();
        };
        let headings: Vec<&str> = text.lines().filter(|line| line.starts_with('#') || line.starts_with("**")).map(|line| line.trim_start_matches(['#', '*', '`', ' ']).trim()).collect();
        let after = after.split_whitespace().collect::<Vec<_>>().join(" ");
        let (mut missing, mut parts) = (Vec::new(), after.split('"'));
        // A title's own backticks are left off; the titles end with the sentence, a bracket, or other code.
        while let (Some(between), Some(title)) = (parts.next(), parts.next()) {
            if between.contains(['`', '|', ')']) || between.contains(". ") {
                break;
            }
            let title = title.trim_matches('`');
            if !headings.iter().any(|heading| heading.starts_with(title)) {
                missing.push(title.to_string());
            }
        }
        missing
    }
}

/// Whether `span` is written as a path: no space in it, a `/` or a
/// file's ending, and nothing that makes it a pattern, a command or a
/// place outside the repository.
fn is_a_path(span: &str) -> bool {
    let plain = !span.contains(|letter: char| letter.is_whitespace() || "<>*{}$~=:(,".contains(letter)) && !span.starts_with('-') && !span.starts_with('/');
    let kept = !NOT_LOOKED_IN.iter().any(|folder| span.split('/').any(|part| part == *folder));
    plain && kept && (span.contains('/') || FILE_ENDINGS.iter().any(|ending| span.ends_with(ending))) && span.chars().any(|letter| letter.is_ascii_alphabetic())
}

/// The names `span` writes, if it is written as code: a path of names
/// joined by `::` or `.`, with or without what it is called with --
/// and looking like code, not a word: a `::`, a `_`, a call, or a
/// capital in it.
fn names_of(span: &str) -> Option<Vec<&str>> {
    let called = span.split('(').next().unwrap_or_default().split('<').next().unwrap_or_default().trim_start_matches('&');
    let like_code = called.contains("::") || called.contains('_') || span.contains('(') || called.chars().any(|letter| letter.is_ascii_uppercase());
    let parts: Vec<&str> = called.split("::").flat_map(|part| part.split('.')).collect();
    (like_code && parts.iter().all(|part| is_a_name(part))).then_some(parts)
}

/// Every markdown file of the repository: its links lead to files
/// that are there, the paths it writes in backticks are there, and
/// the names it writes in backticks are names the code has.
#[test]
fn the_docs_name_what_is_there() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = Repository::read(root);
    let mut docs = Vec::new();
    files(root, &[".md"], &mut docs);
    assert!(docs.len() > 30, "{} docs found: the walk is broken", docs.len());
    let mut wrong = Vec::new();
    for doc in &docs {
        let text = fs::read_to_string(doc).expect("read");
        let shown = doc.strip_prefix(root).expect("under the root").display().to_string();
        let lines = prose(&text);
        for (place, &(number, line)) in lines.iter().enumerate() {
            for link in links(line) {
                let target = link.split('#').next().unwrap_or_default();
                if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") && !doc.parent().expect("in a folder").join(target).exists() {
                    wrong.push(format!("{shown}:{number}: the link to `{link}` leads nowhere"));
                }
            }
            for span in spans(line) {
                if is_a_path(span) {
                    if !repository.has_path(doc, span) {
                        wrong.push(format!("{shown}:{number}: no file or folder `{span}`"));
                    } else if span.ends_with(".md") && span.contains('/') {
                        let after = line.split_once(&format!("`{span}`")).map_or("", |(_, after)| after);
                        let next = lines.get(place + 1).map_or("", |&(_, next)| next);
                        for title in repository.sections_missing(doc, span, &format!("{after} {next}")) {
                            wrong.push(format!("{shown}:{number}: `{span}` has no section \"{title}\""));
                        }
                    }
                } else if let Some(parts) = names_of(span) {
                    for part in parts.into_iter().filter(|part| !repository.names.contains(*part) && !NAMES_NOT_OURS.contains(part)) {
                        wrong.push(format!("{shown}:{number}: `{span}`: the code has no `{part}`"));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} places where the docs name what is not there:\n{}", wrong.len(), wrong.join("\n"));
}

/// The names of the items `source` has, private ones too: what
/// follows the kind of item, a name each. Not what a trait makes a
/// type have -- an `impl ... for` block's -- nor the tests at its end.
fn item_names(source: &str) -> Vec<&str> {
    const KINDS: [&str; 8] = ["fn", "struct", "enum", "trait", "type", "const", "static", "mod"];
    let (mut names, mut within_a_traits) = (Vec::new(), None);
    for line in source.lines() {
        let indented = line.len() - line.trim_start().len();
        let line = line.trim_start();
        if line.starts_with("#[cfg(test)]") {
            break;
        }
        // A trait's block for a type runs to the brace closing it, as far in as it began.
        if let Some(began) = within_a_traits {
            within_a_traits = (indented != began || !line.starts_with('}')).then_some(began);
            continue;
        }
        if line.starts_with("impl") && line.contains(" for ") && line.ends_with('{') {
            within_a_traits = Some(indented);
            continue;
        }
        let shown_to = line.strip_prefix("pub").map_or(line, |after| after.trim_start_matches(|letter: char| letter != ' ').trim_start());
        let mut words: Vec<&str> = shown_to.split_whitespace().collect();
        while words.len() > 2 && ["const", "unsafe"].contains(&words[0]) && words[1] == "fn" || words.first() == Some(&"unsafe") {
            words.remove(0);
        }
        if let [kind, name, ..] = words[..]
            && KINDS.contains(&kind)
        {
            names.push(name.split(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_')).next().unwrap_or_default());
        }
    }
    names.retain(|name| !name.is_empty() && *name != "_");
    names
}

/// Every crate with docs names in them every item it has, private
/// ones too: a reference goes function by function, and none is left
/// out.
#[test]
fn every_item_is_in_its_crates_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = Repository::read(root);
    let mut missing = Vec::new();
    for own in repository.crates.iter().filter(|own| own.join("docs").is_dir() && *own != root) {
        let (mut docs, mut sources) = (Vec::new(), Vec::new());
        files(own, &[".md"], &mut docs);
        files(&own.join("src"), &[".rs"], &mut sources);
        let said: String = docs.iter().filter(|doc| crate_of(doc) == own).map(|doc| fs::read_to_string(doc).unwrap_or_default()).collect();
        let said: HashSet<&str> = said.split(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_')).collect();
        for source in &sources {
            let text = fs::read_to_string(source).expect("read");
            let shown = source.strip_prefix(root).expect("under the root").display().to_string();
            missing.extend(item_names(&text).into_iter().filter(|name| !said.contains(name)).map(|name| format!("{shown}: `{name}`")));
        }
    }
    assert!(missing.is_empty(), "{} items no doc of their crate names:\n{}", missing.len(), missing.join("\n"));
}

/// Every doc a comment in the code points at is there, and has the
/// section the comment names.
#[test]
fn the_code_points_at_docs_that_are_there() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = Repository::read(root);
    let mut sources = Vec::new();
    files(root, &[".rs"], &mut sources);
    let mut wrong = Vec::new();
    for source in &sources {
        let text = fs::read_to_string(source).expect("read");
        let shown = source.strip_prefix(root).expect("under the root").display().to_string();
        let comments: Vec<(usize, &str)> = text.lines().enumerate().filter_map(|(number, line)| line.split_once("//").map(|(_, comment)| (number + 1, comment.trim_start_matches(['/', '!'])))).collect();
        for (place, &(number, comment)) in comments.iter().enumerate() {
            for span in spans(comment).into_iter().filter(|span| span.ends_with(".md") && is_a_path(span)) {
                let from_the_crate = span.trim_start_matches("../");
                let Some(doc) = [crate_of(source), root].iter().map(|from| from.join(from_the_crate)).find(|there| there.exists()) else {
                    wrong.push(format!("{shown}:{number}: no doc `{span}`"));
                    continue;
                };
                let after = comment.split_once(&format!("`{span}`")).map_or("", |(_, after)| after);
                let next = comments.get(place + 1).filter(|(following, _)| *following == number + 1).map_or("", |&(_, next)| next);
                let path_from_the_root = doc.strip_prefix(root).expect("under the root").to_string_lossy().into_owned();
                for title in repository.sections_missing(&root.join("README.md"), &path_from_the_root, &format!("{after} {next}")) {
                    wrong.push(format!("{shown}:{number}: `{span}` has no section \"{title}\""));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} comments pointing at docs that are not there:\n{}", wrong.len(), wrong.join("\n"));
}

/// The crates rules are written in: what they depend on is
/// `instructions` alone, and they ask their turn nothing themselves.
const RULES_CRATES: [&str; 2] = ["sca_rules", "entity_rules"];

/// A rule asks instructions, and nothing else: no rule's source calls
/// anything of its turn's own -- the turn is handed on to an
/// instruction, never asked -- nor names a crate under the
/// instructions (`instructions/docs/instructions.md`, "Rules ask
/// instructions, and nothing else").
#[test]
fn the_rules_ask_instructions_alone() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let under = ["simulation::", "bitplane_manager::", "entity_manager::", "chunk_storage::", "coordinates::", "type_registry::", "utilities::", "pathfinding::", "worldgen::", "server::"];
    let mut wrong = Vec::new();
    for rules in RULES_CRATES {
        let mut sources = Vec::new();
        files(&root.join(rules).join("src"), &[".rs"], &mut sources);
        assert!(!sources.is_empty(), "{rules} has no sources");
        for source in &sources {
            let text = fs::read_to_string(source).expect("read");
            let shown = source.strip_prefix(root).expect("under the root").display().to_string();
            for (number, line) in text.lines().enumerate() {
                // What is said in a comment is not asked.
                let code = line.split_once("//").map_or(line, |(code, _)| code);
                if code.contains("turn.") {
                    wrong.push(format!("{shown}:{}: asks its turn itself: `{}`", number + 1, code.trim()));
                }
                if let Some(named) = under.iter().find(|named| code.contains(**named) && !code.contains(&format!("instructions::{named}"))) {
                    wrong.push(format!("{shown}:{}: names `{}`, under the instructions", number + 1, named.trim_end_matches(':')));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} places where a rule reaches past the instructions:\n{}", wrong.len(), wrong.join("\n"));
}
