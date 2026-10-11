//! The fine tier: the registry's own rows hold together, and its check
//! finds what it is there to find.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use type_registry::{data_words, first_clash, layer_types, named, of_number, AttributeType, EntityType, Kind, Layout, LayerType, Registered, SheepState, GRASS, NUMBERS, REGISTRY, SHEEP, SHEEP_STATE, TREE_STAGE};

/// A row of `kind` named `name` at `number`, `width` wide.
fn row(name: &'static str, kind: Kind, number: u64, width: u32) -> Registered {
    Registered { name, kind, number, width, varies_in_size: false }
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
        for number in registered.number..registered.number + registered.numbers() {
            assert_eq!(of_number(number), Some(registered), "{number}");
        }
    }
    let taken: u64 = REGISTRY.iter().map(Registered::numbers).sum();
    assert_eq!((0..1_000).filter(|&number| of_number(number).is_some()).count() as u64, taken);

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

/// An ID says what it is of in its first byte and its size in its
/// last -- an attribute's blocks, a plane's bits a cell, nothing of an
/// entity type -- and keeps its number between; an attribute's row
/// says its data's width; and a layout's fields come back from the
/// words they were written to.
#[test]
fn an_attribute_type_says_its_blocks_and_a_layout_its_fields() {
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    // The fewest blocks and the most, and every count between with a number drawn: any that fits between the two bytes.
    for blocks in 1..=type_registry::ATTRIBUTE_BLOCKS_MOST {
        let number = 1 + random.below(NUMBERS - 1);
        let kind = AttributeType::of_blocks(number, blocks);
        assert_eq!((kind.blocks(), kind.number(), kind.0 & 0xF, kind.0 >> 56, kind.is_an_attributes()), (blocks, number, blocks as u64 - 1, 0x00, true));
        let varying = AttributeType::of_varying_size(number);
        // One whose size varies is registered with its four bits 0; an attribute of it so many blocks long says so there, and is of the same type.
        assert_eq!((varying.0 & 0xF, varying.number(), varying.is_an_attributes()), (0, number, true));
        assert_eq!((varying.with_blocks(blocks).blocks(), varying.with_blocks(blocks).identity()), (blocks, varying.identity()));
        let (bits, first) = (1u32 << random.below(5), random.below(NUMBERS - 16));
        let plane = LayerType::wide(first, bits);
        assert_eq!((plane.bits(), plane.number(), plane.0 & 0xFF, plane.0 >> 56), (bits, first, u64::from(bits), 0x01));
        assert!((0..bits).all(|bit| plane.holds(plane.plane(bit)) && plane.plane(bit) == LayerType::of_a_bit(first + u64::from(bit))) && !plane.holds(plane.plane(bits)));
        let entity = EntityType::of(number);
        assert_eq!((entity.number(), entity.0 & 0xFF, entity.0 >> 56), (number, 0, 0x02));
    }
    // An ID of another supertype is no attribute's: the one namespace is everything's.
    for other in [GRASS.0, SHEEP.0, TREE_STAGE.layer_type().0, 0] {
        assert!(!AttributeType(other).is_an_attributes());
    }
    let (kind, row) = (SHEEP_STATE.attribute_type(), named("sheep state").expect("registered"));
    assert_eq!((kind.blocks(), row.number, row.width, row.varies_in_size), (3, kind.number(), data_words(3) as u32 * u64::BITS, false));
    assert_eq!(of_number(kind.number()), Some(row));
    for _ in 0..100 {
        let state = SheepState { hungry_at: random.draw(), lamb_due_at: random.draw(), grown_at: random.draw(), roaming_until: random.draw(), roaming_neighbour: random.draw() as u32 };
        let mut data = [0; data_words(SheepState::BLOCKS)];
        state.write(&mut data);
        assert_eq!((SheepState::read(&data), data.len()), (state, SheepState::WORDS));
        // Each member is where the layout says: its bytes, read alone, are the field's.
        let members: Vec<u64> = SheepState::MEMBERS.iter().map(|member| member.of(&data)).collect();
        assert_eq!(members, [state.hungry_at, state.lamb_due_at, state.grown_at, state.roaming_until, u64::from(state.roaming_neighbour)]);
        let (number, mut word) = (random.draw(), [0; data_words(<u64 as Layout>::BLOCKS)]);
        number.write(&mut word);
        assert_eq!((<u64 as Layout>::read(&word), word[1..].iter().sum::<u64>()), (number, 0));
    }
}
