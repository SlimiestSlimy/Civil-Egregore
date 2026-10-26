//! A smoke test: one file of most of the format, read as written.

use utilities::cdnf::{Document, Element, Section};

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

/// `cdnf_smoke.cdnf` parses into the tree it is written as.
#[test]
fn the_smoke_file_reads_as_written() {
    let read = utilities::cdnf::parse(include_str!("cdnf_smoke.cdnf")).expect("parsed");
    let deeper = section("deeper", vec![pair(text("key"), text("value after a blank line"))], Vec::new());
    let lists = section(
        "lists",
        vec![
            pair(text("items"), list(vec![text("one"), pair(text("two"), text("paired")), text("three\non two lines"), pair(text("nested"), list(vec![text("inner one"), text("inner two")])), text("back in the outer list"), pair(text("spaced out"), text("xd")), pair(text("two deep"), list(vec![pair(text("one deep"), list(vec![text("c1"), text("c2")]))])), text("back after two lists ended at once"), pair(text("nested again"), list(vec![text("inner three")]))])),
            text("after the list"),
        ],
        vec![deeper],
    );
    let quotes = section("quotes", vec![pair(text("a quoted key: with a colon"), text("a value\non two lines, \"escaped\" and C:\\games\\")), pair(text("he said \"hi\""), text("a quote mid-text is text"))], Vec::new());
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
    assert_eq!(read, Document { elements: vec![text("before any section\non two lines")], sections: vec![first, second] });
}

/// A section's line indented: refused, its colons at the line's very
/// start or none.
#[test]
fn an_indented_section_is_refused() {
    let refusal = utilities::cdnf::parse("  ::section::\n").expect_err("an indented section");
    assert_eq!(refusal.line, 1);
}
