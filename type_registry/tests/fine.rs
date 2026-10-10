//! The fine tier: the registry's own rows hold together, and its check
//! finds what it is there to find.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use type_registry::{data_words, first_clash, layer_types, named, of_id, AttributeType, Kind, Layout, Registered, Roaming, GRASS, HUNGRY_AT, REGISTRY, ROAMING, TREE_STAGE};

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

/// An attribute's type says its blocks in its top byte and keeps its
/// number under it; its row says its data's width; and a layout's
/// fields come back from the words they were written to.
#[test]
fn an_attribute_type_says_its_blocks_and_a_layout_its_fields() {
    for blocks in [1, 2, 254] {
        let kind = AttributeType::of_blocks(77, blocks);
        assert_eq!((kind.blocks(), kind.number(), kind.0 >> 56, kind.is_an_attributes()), (Some(blocks), 77, blocks as u64, true));
    }
    let varying = AttributeType::of_varying_size(77);
    assert_eq!((varying.blocks(), varying.number(), varying.is_an_attributes()), (None, 77, true));
    // An ID whose top byte is 0 is of something else: the one namespace is everything's.
    assert_eq!((AttributeType(GRASS.0).blocks(), AttributeType(GRASS.0).is_an_attributes()), (Some(0), false));
    for (kind, name) in [(HUNGRY_AT.attribute_type(), "hungry at"), (ROAMING.attribute_type(), "roaming")] {
        let row = named(name).expect("registered");
        assert_eq!((kind.blocks(), row.id, row.width), (Some(1), kind.number(), data_words(1) as u32 * u64::BITS));
        assert_eq!(of_id(kind.number()), Some(row));
    }
    let roaming = Roaming { until: 1 << 40, neighbour: 7 };
    let mut data = [0; data_words(Roaming::BLOCKS)];
    roaming.write(&mut data);
    assert_eq!((Roaming::read(&data), data[2..].iter().sum::<u64>()), (roaming, 0));
}
