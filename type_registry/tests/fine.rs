//! The fine tier: the registry's own rows hold together, and its check
//! finds what it is there to find.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use type_registry::{data_words, default_attribute_of, first_clash, first_default_amiss, first_master_amiss, layer_types, master_of, named, of_number, rule_of_id, rules_of, AttributeType, EntityRuleType, EntityType, Registration, Layout, LayerType, Registered, ScaRuleType, SheepState, GRASS, NUMBERS, REGISTRY, RULES_REGISTERED, CLIMATE, RULE_OF_GRASS, RULE_OF_SHEEP, RULE_OF_TREES, RULE_OF_WEATHER, SHEEP, SHEEP_STATE, TREE, TREE_STAGE};
use utilities::rng::Rng;

/// A row registering `registration`, named `name` at `number`, `width` wide.
fn row(name: &'static str, registration: Registration, number: u64, width: u32) -> Registered {
    Registered { name, registration, number, width, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
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

    let sound = [row("a", Registration::Layer, 2, 1), row("b", Registration::WidePlane, 4, 4), row("c", Registration::Layer, 8, 1), row("d", Registration::EntityType, 3, 0), row("e", Registration::Attribute, 9, 64)];
    assert_eq!(first_clash(&sound), None);
    let clashing = [row("f", Registration::Layer, 4, 1), row("f", Registration::Layer, 7, 1), row("f", Registration::WidePlane, 6, 2), row("f", Registration::Attribute, 3, 64), row("f", Registration::WidePlane, 0, 16), row("a", Registration::Layer, 100, 1), row("f", Registration::WidePlane, 1, 2)];
    for clash in clashing {
        for at in 0..=sound.len() {
            let mut table = sound.to_vec();
            table.insert(at, clash);
            let found = first_clash(&table).unwrap_or_else(|| panic!("{clash:?} put at {at} not caught"));
            assert!(found >= at.min(1), "{clash:?} at {at}: caught at {found}");
        }
    }
    assert_eq!(first_clash(&[row("b", Registration::WidePlane, 4, 4), row("f", Registration::Layer, 8, 1), row("g", Registration::Layer, 3, 1)]), None, "beside a wide plane, not in it");
}

/// The layer types are every layer and wide plane, a wide plane's with
/// its width, and each is found again in a constant.
/// A sheep's default attribute is its state, and a type with none
/// registered has none. In a table drawn from the seed -- entity
/// types and attributes at numbers of its choosing, some attributes
/// varying in size -- a default naming a fixed attribute of the table
/// is sound, and one naming a number the table has not, an attribute
/// whose size varies, or put on a row that is no entity type's, is
/// caught at its row.
#[test]
fn a_default_attribute_is_an_attribute_of_a_fixed_size_of_the_table() {
    assert_eq!(first_default_amiss(REGISTRY), None);
    assert_eq!(default_attribute_of(SHEEP), named("sheep state"));
    assert_eq!(default_attribute_of(EntityType::from_number(200_000)), None, "a type not registered");
    assert!(REGISTRY.iter().all(|registered| registered.registration == Registration::EntityType || registered.default_attribute == 0));
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..200 {
        let rows = 4 + random.below(12) as usize;
        // Numbers apart, each row an entity type or an attribute, the first two one of each: there is always one of either to name.
        let mut table: Vec<Registered> = (0..rows)
            .map(|at| {
                let attribute = at == 1 || (at != 0 && random.below(2) == 0);
                Registered { varies_in_size: attribute && at != 1 && random.below(3) == 0, ..row("drawn", if attribute { Registration::Attribute } else { Registration::EntityType }, 100 + 10 * at as u64 + random.below(10), 64) }
            })
            .collect();
        assert_eq!(first_default_amiss(&table), None, "none has a default");
        let of_kind = |table: &[Registered], registration: Registration, varying: bool, random: &mut Rng| {
            let found: Vec<usize> = (0..table.len()).filter(|&at| table[at].registration == registration && table[at].varies_in_size == varying).collect();
            (!found.is_empty()).then(|| found[random.below(found.len() as u64) as usize])
        };
        let entity = of_kind(&table, Registration::EntityType, false, &mut random).expect("the first row");
        let fixed = of_kind(&table, Registration::Attribute, false, &mut random).expect("the second row");
        table[entity].default_attribute = table[fixed].number;
        assert_eq!(first_default_amiss(&table), None, "an attribute of a fixed size of the table");
        // Amiss, each in its way: a number no row has, an attribute whose size varies, a row that is no entity type's.
        let varying = of_kind(&table, Registration::Attribute, true, &mut random);
        let amiss = [Some((entity, 5)), varying.map(|varying| (entity, table[varying].number)), Some((fixed, table[fixed].number))];
        for (at, default) in amiss.into_iter().flatten() {
            let mut table = table.clone();
            table[at].default_attribute = default;
            // The entity type's own default being sound, the one amiss is the row changed.
            assert_eq!(first_default_amiss(&table), Some(at), "row {at} given {default}");
        }
    }
}

/// Every rule registered is found by its ID, of its own kind in the
/// ID's first byte, and has the master it was written with -- a plane
/// a rule of the cells, a type of entity an entity's -- which names
/// it among its rules; no other row has a master. In a table drawn
/// from the seed -- planes, entity types and attributes at numbers of
/// its choosing -- a rule of the cells whose master is a plane of the
/// table and a rule of an entity whose master is an entity type of it
/// are sound, and a rule whose master is of the other kind, an
/// attribute, a number the table has not, or none, and a row that is
/// no rule and has one, are each caught at their row.
#[test]
fn a_rule_has_one_master_of_the_kind_its_rule_is_of() {
    assert_eq!(first_master_amiss(REGISTRY), None);
    let rules = [(RULE_OF_GRASS.0, "grass rule", GRASS.number()), (RULE_OF_TREES.0, "trees rule", TREE.number()), (RULE_OF_SHEEP.0, "sheep rule", SHEEP.number()), (RULE_OF_WEATHER.0, "weather rule", CLIMATE.layer_type().number())];
    assert_eq!(rules.len(), RULES_REGISTERED);
    for (id, name, master) in rules {
        let rule = rule_of_id(id).unwrap_or_else(|| panic!("{name} not found by its ID"));
        assert_eq!((rule.name, rule.master, named(name)), (name, master, Some(rule)));
        let of = master_of(rule).expect("its master is registered");
        assert_eq!(of.number, master);
        assert!(rules_of(of).any(|other| other == rule), "{name} is not among its master's rules");
    }
    assert_eq!((RULE_OF_GRASS.0 >> 56, RULE_OF_SHEEP.0 >> 56, RULE_OF_GRASS.0 & 0xFF, RULE_OF_SHEEP.0 & 0xFF), (0x03, 0x04, 0, 0));
    // A rule's number read as the other kind's, or as a plane's, is no rule's ID.
    assert_eq!((rule_of_id(EntityRuleType::from_number(RULE_OF_GRASS.number()).0), rule_of_id(ScaRuleType::from_number(RULE_OF_SHEEP.number()).0), rule_of_id(GRASS.0), rule_of_id(SHEEP.0)), (None, None, None, None));
    for registered in REGISTRY {
        let rule = matches!(registered.registration, Registration::ScaRule | Registration::EntityRule);
        assert_eq!((registered.master != 0, master_of(registered).is_some()), (rule, rule), "{}", registered.name);
        assert_eq!(rules_of(registered).count(), rules.iter().filter(|rule| rule.2 == registered.number).count(), "{}", registered.name);
    }
    let mut random = Rng::new(utilities::seed::counted());
    for _ in 0..200 {
        let rows = 5 + random.below(12) as usize;
        // Numbers apart; the first four a layer, a wide plane, an entity type and an attribute: there is always one of each to name.
        let kinds = [Registration::Layer, Registration::WidePlane, Registration::EntityType, Registration::Attribute];
        let mut table: Vec<Registered> = (0..rows).map(|at| row("drawn", kinds[if at < 4 { at } else { random.below(4) as usize }], 100 + 10 * at as u64 + random.below(6), 1)).collect();
        assert_eq!(first_master_amiss(&table), None, "no rule, no master");
        let of_kind = |table: &[Registered], registration: Registration, random: &mut Rng| {
            let found: Vec<u64> = table.iter().filter(|registered| registered.registration == registration).map(|registered| registered.number).collect();
            found[random.below(found.len() as u64) as usize]
        };
        let (layer, wide, entity, attribute) = (of_kind(&table, Registration::Layer, &mut random), of_kind(&table, Registration::WidePlane, &mut random), of_kind(&table, Registration::EntityType, &mut random), of_kind(&table, Registration::Attribute, &mut random));
        let rule = |registration, master| Registered { master, ..row("a rule", registration, 5, 0) };
        // Sound: a rule of the cells of a layer or of a wide plane, an entity's of an entity type. Put anywhere among the rows.
        for sound in [rule(Registration::ScaRule, layer), rule(Registration::ScaRule, wide), rule(Registration::EntityRule, entity)] {
            let mut with = table.clone();
            with.insert(random.below(rows as u64 + 1) as usize, sound);
            assert_eq!(first_master_amiss(&with), None, "{sound:?}");
        }
        // Amiss, each in its way: of the other kind, an attribute, a number no row has, none, its own number.
        let amiss = [rule(Registration::ScaRule, entity), rule(Registration::EntityRule, layer), rule(Registration::EntityRule, wide), rule(Registration::ScaRule, attribute), rule(Registration::EntityRule, attribute), rule(Registration::ScaRule, 7), rule(Registration::EntityRule, 0), rule(Registration::ScaRule, 5)];
        for amiss in amiss {
            let at = random.below(rows as u64 + 1) as usize;
            let mut with = table.clone();
            with.insert(at, amiss);
            assert_eq!(first_master_amiss(&with), Some(at), "{amiss:?}");
        }
        // A row that is no rule, given a master: caught, whatever it names.
        let at = random.below(rows as u64) as usize;
        table[at].master = [layer, wide, entity, attribute][random.below(4) as usize];
        assert_eq!(first_master_amiss(&table), Some(at), "{:?}", table[at]);
    }
}

#[test]
fn the_layer_types_are_the_layers_and_wide_planes() {
    let types = layer_types();
    assert_eq!(types.len(), REGISTRY.iter().filter(|registered| matches!(registered.registration, Registration::Layer | Registration::WidePlane)).count());
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
        let entity = EntityType::from_number(number);
        assert_eq!((entity.number(), entity.0 & 0xFF, entity.0 >> 56), (number, 0, 0x02));
    }
    // An ID of another kind is no attribute's: the one namespace is everything's.
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
        let members: Vec<u64> = SheepState::MEMBERS.iter().map(|member| member.value_in(&data)).collect();
        assert_eq!(members, [state.hungry_at, state.lamb_due_at, state.grown_at, state.roaming_until, u64::from(state.roaming_neighbour)]);
        let (number, mut word) = (random.draw(), [0; data_words(<u64 as Layout>::BLOCKS)]);
        number.write(&mut word);
        assert_eq!((<u64 as Layout>::read(&word), word[1..].iter().sum::<u64>()), (number, 0));
    }
}
