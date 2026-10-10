//! The fine tier: the registry's own rows hold together, and its check
//! finds what it is there to find.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use type_registry::{first_clash, layer_types, named, of_id, Kind, Registered, GRASS, REGISTRY, TREE_STAGE};

/// A row of `kind` named `name` at `id`, `width` wide.
fn row(name: &'static str, kind: Kind, id: u64, width: u32) -> Registered {
    Registered { name, kind, id, width }
}

/// The registry has no clash; every row is found by its name and by
/// each of its IDs, and by none other; and a table with a plane put
/// where a wide one's cold planes are, an ID used twice, or a name
/// used twice is caught at the row that does it -- wherever in the
/// table the two are.
#[test]
fn the_check_finds_every_clash_and_the_registry_has_none() {
    assert_eq!(first_clash(REGISTRY), None);
    for registered in REGISTRY {
        assert_eq!(named(registered.name), Some(registered));
        for id in registered.id..registered.id + registered.ids() {
            assert_eq!(of_id(id), Some(registered), "{id}");
        }
    }
    let taken: u64 = REGISTRY.iter().map(Registered::ids).sum();
    assert_eq!((0..1_000).filter(|&id| of_id(id).is_some()).count() as u64, taken);

    let sound = [row("a", Kind::Layer, 2, 1), row("b", Kind::WidePlane, 4, 4), row("c", Kind::Layer, 8, 1), row("d", Kind::EntityType, 3, 0), row("e", Kind::Attribute, 9, 64)];
    assert_eq!(first_clash(&sound), None);
    let clashing = [row("f", Kind::Layer, 4, 1), row("f", Kind::Layer, 7, 1), row("f", Kind::WidePlane, 6, 2), row("f", Kind::Attribute, 3, 64), row("f", Kind::WidePlane, 0, 16), row("a", Kind::Layer, 100, 1), row("f", Kind::WidePlane, 1, 2)];
    for clash in clashing {
        for at in 0..=sound.len() {
            let mut table = sound.to_vec();
            table.insert(at, clash);
            let found = first_clash(&table).unwrap_or_else(|| panic!("{clash:?} put at {at} not caught"));
            assert!(found >= at.min(1), "{clash:?} at {at}: caught at {found}");
        }
    }
    assert_eq!(first_clash(&[row("b", Kind::WidePlane, 4, 4), row("f", Kind::Layer, 8, 1), row("g", Kind::Layer, 3, 1)]), None, "beside a wide plane, not in it");
}

/// The layer types are every layer and wide plane, a wide plane's with
/// its width, and each is found again in a constant.
#[test]
fn the_layer_types_are_the_layers_and_wide_planes() {
    let types = layer_types();
    assert_eq!(types.len(), REGISTRY.iter().filter(|registered| matches!(registered.kind, Kind::Layer | Kind::WidePlane)).count());
    assert!(types.contains(&GRASS) && types.contains(&TREE_STAGE.layer_type()));
    assert_eq!(TREE_STAGE.layer_type().bits(), named("tree stage").expect("registered").width);
}
