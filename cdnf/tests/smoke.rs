//! The format's tests: `cdnf.cdnf` holds all that is allowed, and reads;
//! each file of `smokefail/` holds something that isn't, and is refused. Then
//! what a few texts read as exactly, and what the writer writes.

use cdnf::writer::CdnfWriter;
use cdnf::{CdnfError, Document, Element, Section};

/// A text.
fn text(text: &str) -> Element {
    Element::Text(text.to_string())
}

/// A pair.
fn pair(left: Element, right: Element) -> Element {
    Element::Pair(Box::new(left), Box::new(right))
}

/// A list.
fn list(items: Vec<Element>) -> Element {
    Element::List(items)
}

/// A section.
fn section(name: &str, elements: Vec<Element>, sections: Vec<Section>) -> Section {
    Section { name: name.to_string(), elements, sections }
}

/// The elements `text` reads as.
fn elements(text: &str) -> Vec<Element> {
    cdnf::parse(text).expect("parsed").elements
}

/// `cdnf.cdnf`, the rules written in them, reads as they say.
#[test]
fn the_rules_file_reads() {
    cdnf::parse(include_str!("../cdnf.cdnf")).expect("the rules read");
}

/// Every file of `smokefail/` is refused.
#[test]
fn every_smokefail_file_is_refused() {
    let smokefail_folder = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/smokefail");
    let files: Vec<_> = std::fs::read_dir(smokefail_folder).expect("the smokefail folder").map(|file| file.expect("a file").path()).collect();
    assert!(!files.is_empty(), "no smokefail file");
    for file in files {
        let smoke = std::fs::read_to_string(&file).expect("a smokefail file");
        assert!(cdnf::parse(&smoke).is_err(), "{} read", file.display());
    }
}

/// A colon or a dash with no space after it is text; a comment is
/// nothing, wherever it is, a colon before it still making a pair; any
/// blank a reader sees as one is a space.
#[test]
fn text_needs_no_quotes() {
    assert_eq!(elements("a time : 12:54\n"), [pair(text("a time"), text("12:54"))]);
    assert_eq!(elements("-5\n--x\n---\n"), [text("-5\n--x\n---")]);
    assert_eq!(elements("a color : #ff8800\n"), [pair(text("a color"), text("#ff8800"))]);
    assert_eq!(elements("sample:<:->comment\nright\n"), [pair(text("sample"), text("right"))]);
    assert_eq!(elements("this<:->that\n"), [text("this")]);
    assert_eq!(elements("\u{feff}a\u{a0}:\u{a0}b\r\n"), [pair(text("a"), text("b"))]);
}

/// Section names follow the rules of any text.
#[test]
fn section_names_are_text_bodies() {
    let name = |text: &str| cdnf::parse(text).expect("parsed").sections[0].name.clone();
    assert_eq!(name(":12:54:\n"), "12:54");
    assert_eq!(name(":\"a: b::\": <:-> a comment\n"), "a: b::");
    assert_eq!(name(":  spaced  :\n"), "spaced");
}

/// A quoted text: only `\"` escapes.
#[test]
fn only_a_quote_escapes() {
    assert_eq!(elements("\"a\\b\\\\c\"\n"), [text("a\\b\\\\c")]);
    assert_eq!(elements("\"a\\\\\" b\"\n"), [text("a\\\" b")]);
}

/// A text that needs quotes and ends in a backslash is refused, wherever
/// it goes; one that needs none is written.
#[test]
fn a_quoted_last_backslash_is_refused() -> Result<(), CdnfError> {
    let writer = CdnfWriter::new;
    assert!(writer().text("a: b\\").is_err());
    assert!(writer().pair("a: b\\").is_err());
    assert!(writer().section("a: b\\").is_err());
    assert!(writer().pair("a")?.text("a: b\\").is_err());
    assert_eq!(elements(&writer().text("C:\\games\\")?.finish()), [text("C:\\games\\")]);
    Ok(())
}

/// A section's name no line can hold is refused, on the line it would
/// open.
#[test]
fn names_no_line_holds_are_refused() -> Result<(), CdnfError> {
    for name in ["", "a\nb"] {
        let refusal = CdnfWriter::new().text("before")?.section(name).err().expect("refused");
        assert_eq!(refusal.line, 3);
    }
    Ok(())
}

/// Texts that look like marks, written, read back as they are.
#[test]
fn awkward_texts_are_written_back() -> Result<(), CdnfError> {
    for each in ["", "- a", "-- a", "-", "--", "a: b", "a:", ":a", "12:54", "-5", "\"a", "a\"b", "<:->", "a<:->b", " a", "a\n\nb", "a\n- b", "a\n: b", "\\\"", "#ff8800"] {
        let written = CdnfWriter::new().text(each)?.finish();
        assert_eq!(elements(&written), [text(each)], "{written}");
    }
    Ok(())
}

/// A tree of most of the format, built by the writer, reads back as
/// built: a list of two nested lists ending at once among it.
#[test]
fn a_written_tree_reads_back() -> Result<(), CdnfError> {
    let written = CdnfWriter::new()
        .text("before any section\non two lines")?
        .section("first")?
            .pair("name")?.text("smoke")?
            .pair("a")?.pair("b")?.text("c")?
            .pair("a body\ngoes on")?.text("and its pair\nindented lines\nread alike")?
            .section("lists")?
                .pair("items")?.list()
                    .text("one")?
                    .pair("two")?.text("paired")?
                    .text("three\non two lines")?
                    .pair("nested")?.list().text("inner one")?.text("inner two")?.end_list()
                    .text("back in the outer list")?
                    .pair("spaced out")?.text("xd")?
                    .pair("two deep")?.list().pair("one deep")?.list().text("c1")?.text("c2")?.end_list().end_list()
                    .text("back after two lists ended at once")?
                    .pair("nested again")?.list().text("inner three")?.end_list()
                .end_list()
                .text("after the list")?
                .section("deeper")?
                    .pair("key")?.text("value after a blank line")?
                .end_section()
            .end_section()
            .section("quotes")?
                .pair("a quoted key: with a colon")?.text("a value\non two lines, \"escaped\" and C:\\games\\ on")?
                .pair("he said \"hi\"")?.text("a quote mid-text is text")?
            .end_section()
        .end_section()
        .section("second")?
            .list().text("last")?.end_list()
        .end_section()
        .finish();
    let deeper = section("deeper", vec![pair(text("key"), text("value after a blank line"))], Vec::new());
    let lists = section(
        "lists",
        vec![
            pair(text("items"), list(vec![text("one"), pair(text("two"), text("paired")), text("three\non two lines"), pair(text("nested"), list(vec![text("inner one"), text("inner two")])), text("back in the outer list"), pair(text("spaced out"), text("xd")), pair(text("two deep"), list(vec![pair(text("one deep"), list(vec![text("c1"), text("c2")]))])), text("back after two lists ended at once"), pair(text("nested again"), list(vec![text("inner three")]))])),
            text("after the list"),
        ],
        vec![deeper],
    );
    let quotes = section("quotes", vec![pair(text("a quoted key: with a colon"), text("a value\non two lines, \"escaped\" and C:\\games\\ on")), pair(text("he said \"hi\""), text("a quote mid-text is text"))], Vec::new());
    let first = section(
        "first",
        vec![
            pair(text("name"), text("smoke")),
            pair(text("a"), pair(text("b"), text("c"))),
            pair(text("a body\ngoes on"), text("and its pair\nindented lines\nread alike")),
        ],
        vec![lists, quotes],
    );
    let second = section("second", vec![list(vec![text("last")])], Vec::new());
    assert_eq!(cdnf::parse(&written).expect("written parses"), Document { elements: vec![text("before any section\non two lines")], sections: vec![first, second] }, "{written}");
    Ok(())
}
