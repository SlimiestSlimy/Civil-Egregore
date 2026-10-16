//! The fine tier: the registry's own rows hold together, and its check
//! finds what it is there to find.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use type_registry::{data_words, default_attribute_of, first_clash, first_default_amiss, first_master_amiss, layer_types, master_of, row_named, of_number, rule_of_id, rules_of, AttributeType, EntityRuleType, EntityType, Registration, Layout, LayerType, RegistryRow, ScaRuleType, SheepState, GRASS, TYPE_ID_NUMBERS, REGISTRY, RULES_REGISTERED, CLIMATE, RULE_OF_GRASS, RULE_OF_SHEEP, RULE_OF_TREES, RULE_OF_WEATHER, SHEEP, SHEEP_STATE, TREE, TREE_STAGE};
use utilities::rng::RandomSource;

/// A row registering `registration`, named `name` at `type_number`, `width` wide.
fn test_row(name: &'static str, registration: Registration, type_number: u64, width: u32) -> RegistryRow {
    RegistryRow { name, registration, type_number, width, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// The registry has no clash; every row is found by its name and by
/// each of its IDs, and by none other; and a table with a plane put
/// where a wide one's cold planes are, an ID used twice, or a name
/// used twice is caught at the row that does it -- wherever in the
/// table the two are.
#[test]
fn the_check_finds_every_clash_and_the_registry_has_none() {
    assert_eq!(first_clash(REGISTRY), None);
    for registered_row in REGISTRY {
        assert_eq!(row_named(registered_row.name), Some(registered_row));
        for type_number in registered_row.type_number..registered_row.type_number + registered_row.numbers_taken() {
            assert_eq!(of_number(type_number), Some(registered_row), "{type_number}");
        }
    }
    let total_taken: u64 = REGISTRY.iter().map(RegistryRow::numbers_taken).sum();
    assert_eq!((0..1_000).filter(|&type_number| of_number(type_number).is_some()).count() as u64, total_taken);

    let sound_table = [test_row("a", Registration::Layer, 2, 1), test_row("b", Registration::WidePlane, 4, 4), test_row("c", Registration::Layer, 8, 1), test_row("d", Registration::EntityType, 3, 0), test_row("e", Registration::Attribute, 9, 64)];
    assert_eq!(first_clash(&sound_table), None);
    let clashing_rows = [test_row("f", Registration::Layer, 4, 1), test_row("f", Registration::Layer, 7, 1), test_row("f", Registration::WidePlane, 6, 2), test_row("f", Registration::Attribute, 3, 64), test_row("f", Registration::WidePlane, 0, 16), test_row("a", Registration::Layer, 100, 1), test_row("f", Registration::WidePlane, 1, 2)];
    for clashing_row in clashing_rows {
        for insert_index in 0..=sound_table.len() {
            let mut table = sound_table.to_vec();
            table.insert(insert_index, clashing_row);
            let found_row = first_clash(&table).unwrap_or_else(|| panic!("{clashing_row:?} put at {insert_index} not caught"));
            assert!(found_row >= insert_index.min(1), "{clashing_row:?} at {insert_index}: caught at {found_row}");
        }
    }
    assert_eq!(first_clash(&[test_row("b", Registration::WidePlane, 4, 4), test_row("f", Registration::Layer, 8, 1), test_row("g", Registration::Layer, 3, 1)]), None, "beside a wide plane, not in it");
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
    assert_eq!(default_attribute_of(SHEEP), row_named("sheep state"));
    assert_eq!(default_attribute_of(EntityType::from_number(200_000)), None, "a type not registered");
    assert!(REGISTRY.iter().all(|registered_row| registered_row.registration == Registration::EntityType || registered_row.default_attribute == 0));
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    for _ in 0..200 {
        let row_count = 4 + seeded_rng.draw_below(12) as usize;
        // Numbers apart, each row an entity type or an attribute, the first two one of each: there is always one of either to name.
        let mut table: Vec<RegistryRow> = (0..row_count)
            .map(|row_index| {
                let attribute = row_index == 1 || (row_index != 0 && seeded_rng.draw_below(2) == 0);
                RegistryRow { varies_in_size: attribute && row_index != 1 && seeded_rng.draw_below(3) == 0, ..test_row("drawn", if attribute { Registration::Attribute } else { Registration::EntityType }, 100 + 10 * row_index as u64 + seeded_rng.draw_below(10), 64) }
            })
            .collect();
        assert_eq!(first_default_amiss(&table), None, "none has a default");
        let of_kind = |table: &[RegistryRow], wanted_registration: Registration, wanted_varying: bool, seeded_rng: &mut RandomSource| {
            let found_rows: Vec<usize> = (0..table.len()).filter(|&row_index| table[row_index].registration == wanted_registration && table[row_index].varies_in_size == wanted_varying).collect();
            (!found_rows.is_empty()).then(|| found_rows[seeded_rng.draw_below(found_rows.len() as u64) as usize])
        };
        let entity = of_kind(&table, Registration::EntityType, false, &mut seeded_rng).expect("the first row");
        let fixed_attribute_row = of_kind(&table, Registration::Attribute, false, &mut seeded_rng).expect("the second row");
        table[entity].default_attribute = table[fixed_attribute_row].type_number;
        assert_eq!(first_default_amiss(&table), None, "an attribute of a fixed size of the table");
        // Amiss, each in its way: a number no row has, an attribute whose size varies, a row that is no entity type's.
        let varying_attribute_row = of_kind(&table, Registration::Attribute, true, &mut seeded_rng);
        let amiss_rows = [Some((entity, 5)), varying_attribute_row.map(|varying_row| (entity, table[varying_row].type_number)), Some((fixed_attribute_row, table[fixed_attribute_row].type_number))];
        for (row_index, default_number) in amiss_rows.into_iter().flatten() {
            let mut table = table.clone();
            table[row_index].default_attribute = default_number;
            // The entity type's own default being sound, the one amiss is the row changed.
            assert_eq!(first_default_amiss(&table), Some(row_index), "row {row_index} given {default_number}");
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
    let rules = [(RULE_OF_GRASS.0, "grass rule", GRASS.type_number()), (RULE_OF_TREES.0, "trees rule", TREE.type_number()), (RULE_OF_SHEEP.0, "sheep rule", SHEEP.type_number()), (RULE_OF_WEATHER.0, "weather rule", CLIMATE.layer_type().type_number())];
    assert_eq!(rules.len(), RULES_REGISTERED);
    for (rule_id, rule_name, master) in rules {
        let rule = rule_of_id(rule_id).unwrap_or_else(|| panic!("{rule_name} not found by its ID"));
        assert_eq!((rule.name, rule.master, row_named(rule_name)), (rule_name, master, Some(rule)));
        let master_row = master_of(rule).expect("its master is registered");
        assert_eq!(master_row.type_number, master);
        assert!(rules_of(master_row).any(|other_rule| other_rule == rule), "{rule_name} is not among its master's rules");
    }
    assert_eq!((RULE_OF_GRASS.0 >> 56, RULE_OF_SHEEP.0 >> 56, RULE_OF_GRASS.0 & 0xFF, RULE_OF_SHEEP.0 & 0xFF), (0x03, 0x04, 0, 0));
    // A rule's number read as the other kind's, or as a plane's, is no rule's ID.
    assert_eq!((rule_of_id(EntityRuleType::from_number(RULE_OF_GRASS.type_number()).0), rule_of_id(ScaRuleType::from_number(RULE_OF_SHEEP.type_number()).0), rule_of_id(GRASS.0), rule_of_id(SHEEP.0)), (None, None, None, None));
    for registered_row in REGISTRY {
        let rule = matches!(registered_row.registration, Registration::ScaRule | Registration::EntityRule);
        assert_eq!((registered_row.master != 0, master_of(registered_row).is_some()), (rule, rule), "{}", registered_row.name);
        assert_eq!(rules_of(registered_row).count(), rules.iter().filter(|rule| rule.2 == registered_row.type_number).count(), "{}", registered_row.name);
    }
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    for _ in 0..200 {
        let row_count = 5 + seeded_rng.draw_below(12) as usize;
        // Numbers apart; the first four a layer, a wide plane, an entity type and an attribute: there is always one of each to name.
        let row_registrations = [Registration::Layer, Registration::WidePlane, Registration::EntityType, Registration::Attribute];
        let mut table: Vec<RegistryRow> = (0..row_count).map(|row_index| test_row("drawn", row_registrations[if row_index < 4 { row_index } else { seeded_rng.draw_below(4) as usize }], 100 + 10 * row_index as u64 + seeded_rng.draw_below(6), 1)).collect();
        assert_eq!(first_master_amiss(&table), None, "no rule, no master");
        let of_kind = |table: &[RegistryRow], wanted_registration: Registration, seeded_rng: &mut RandomSource| {
            let found_numbers: Vec<u64> = table.iter().filter(|registered_row| registered_row.registration == wanted_registration).map(|registered_row| registered_row.type_number).collect();
            found_numbers[seeded_rng.draw_below(found_numbers.len() as u64) as usize]
        };
        let (layer, wide_plane_number, entity, attribute) = (of_kind(&table, Registration::Layer, &mut seeded_rng), of_kind(&table, Registration::WidePlane, &mut seeded_rng), of_kind(&table, Registration::EntityType, &mut seeded_rng), of_kind(&table, Registration::Attribute, &mut seeded_rng));
        let rule = |rule_registration, master| RegistryRow { master, ..test_row("a rule", rule_registration, 5, 0) };
        // Sound: a rule of the cells of a layer or of a wide plane, an entity's of an entity type. Put anywhere among the rows.
        for sound_rule in [rule(Registration::ScaRule, layer), rule(Registration::ScaRule, wide_plane_number), rule(Registration::EntityRule, entity)] {
            let mut table_with_rule = table.clone();
            table_with_rule.insert(seeded_rng.draw_below(row_count as u64 + 1) as usize, sound_rule);
            assert_eq!(first_master_amiss(&table_with_rule), None, "{sound_rule:?}");
        }
        // Amiss, each in its way: of the other kind, an attribute, a number no row has, none, its own number.
        let amiss_rules = [rule(Registration::ScaRule, entity), rule(Registration::EntityRule, layer), rule(Registration::EntityRule, wide_plane_number), rule(Registration::ScaRule, attribute), rule(Registration::EntityRule, attribute), rule(Registration::ScaRule, 7), rule(Registration::EntityRule, 0), rule(Registration::ScaRule, 5)];
        for amiss_rule in amiss_rules {
            let insert_index = seeded_rng.draw_below(row_count as u64 + 1) as usize;
            let mut table_with_rule = table.clone();
            table_with_rule.insert(insert_index, amiss_rule);
            assert_eq!(first_master_amiss(&table_with_rule), Some(insert_index), "{amiss_rule:?}");
        }
        // A row that is no rule, given a master: caught, whatever it names.
        let row_index = seeded_rng.draw_below(row_count as u64) as usize;
        table[row_index].master = [layer, wide_plane_number, entity, attribute][seeded_rng.draw_below(4) as usize];
        assert_eq!(first_master_amiss(&table), Some(row_index), "{:?}", table[row_index]);
    }
}

#[test]
fn the_layer_types_are_the_layers_and_wide_planes() {
    let registered_layer_types = layer_types();
    assert_eq!(registered_layer_types.len(), REGISTRY.iter().filter(|registered_row| matches!(registered_row.registration, Registration::Layer | Registration::WidePlane)).count());
    assert!(registered_layer_types.contains(&GRASS) && registered_layer_types.contains(&TREE_STAGE.layer_type()));
    assert_eq!(TREE_STAGE.layer_type().width_bits(), row_named("tree stage").expect("registered").width);
}

/// An ID says what it is of in its first byte and its size in its
/// last -- an attribute's blocks, a plane's bits a cell, nothing of an
/// entity type -- and keeps its number between; an attribute's row
/// says its data's width; and a layout's fields come back from the
/// words they were written to.
#[test]
fn an_attribute_type_says_its_blocks_and_a_layout_its_fields() {
    let mut seeded_rng = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    // The fewest blocks and the most, and every count between with a number drawn: any that fits between the two bytes.
    for block_count in 1..=type_registry::ATTRIBUTE_BLOCKS_MOST {
        let type_number = 1 + seeded_rng.draw_below(TYPE_ID_NUMBERS - 1);
        let drawn_type = AttributeType::of_blocks(type_number, block_count);
        assert_eq!((drawn_type.block_count(), drawn_type.type_number(), drawn_type.0 & 0xF, drawn_type.0 >> 56, drawn_type.is_an_attributes()), (block_count, type_number, block_count as u64 - 1, 0x00, true));
        let varying_type = AttributeType::of_varying_size(type_number);
        // One whose size varies is registered with its four bits 0; an attribute of it so many blocks long says so there, and is of the same type.
        assert_eq!((varying_type.0 & 0xF, varying_type.type_number(), varying_type.is_an_attributes()), (0, type_number, true));
        assert_eq!((varying_type.with_blocks(block_count).block_count(), varying_type.with_blocks(block_count).type_identity()), (block_count, varying_type.type_identity()));
        let (drawn_width, first_number) = (1u32 << seeded_rng.draw_below(5), seeded_rng.draw_below(TYPE_ID_NUMBERS - 16));
        let wide_layer = LayerType::of_width(first_number, drawn_width);
        assert_eq!((wide_layer.width_bits(), wide_layer.type_number(), wide_layer.0 & 0xFF, wide_layer.0 >> 56), (drawn_width, first_number, u64::from(drawn_width), 0x01));
        assert!((0..drawn_width).all(|bit_index| wide_layer.holds_bitplane(wide_layer.bitplane_of(bit_index)) && wide_layer.bitplane_of(bit_index) == LayerType::of_a_bit(first_number + u64::from(bit_index))) && !wide_layer.holds_bitplane(wide_layer.bitplane_of(drawn_width)));
        let entity = EntityType::from_number(type_number);
        assert_eq!((entity.type_number(), entity.0 & 0xFF, entity.0 >> 56), (type_number, 0, 0x02));
    }
    // An ID of another kind is no attribute's: the one namespace is everything's.
    for other_id in [GRASS.0, SHEEP.0, TREE_STAGE.layer_type().0, 0] {
        assert!(!AttributeType(other_id).is_an_attributes());
    }
    let (state_type, state_row) = (SHEEP_STATE.attribute_type(), row_named("sheep state").expect("registered"));
    assert_eq!((state_type.block_count(), state_row.type_number, state_row.width, state_row.varies_in_size), (3, state_type.type_number(), data_words(3) as u32 * u64::BITS, false));
    assert_eq!(of_number(state_type.type_number()), Some(state_row));
    for _ in 0..100 {
        let sheep_state = SheepState { hungry_at: seeded_rng.next_draw(), lamb_due_at: seeded_rng.next_draw(), grown_at: seeded_rng.next_draw(), roaming_until: seeded_rng.next_draw(), roaming_neighbour: seeded_rng.next_draw() as u32 };
        let mut state_data = [0; data_words(SheepState::BLOCK_COUNT)];
        sheep_state.write(&mut state_data);
        assert_eq!((SheepState::read_data(&state_data), state_data.len()), (sheep_state, SheepState::WORDS));
        // Each member is where the layout says: its bytes, read alone, are the field's.
        let members: Vec<u64> = SheepState::MEMBERS.iter().map(|member| member.value_in(&state_data)).collect();
        assert_eq!(members, [sheep_state.hungry_at, sheep_state.lamb_due_at, sheep_state.grown_at, sheep_state.roaming_until, u64::from(sheep_state.roaming_neighbour)]);
        let (drawn_number, mut word) = (seeded_rng.next_draw(), [0; data_words(<u64 as Layout>::BLOCK_COUNT)]);
        drawn_number.write(&mut word);
        assert_eq!((<u64 as Layout>::read_data(&word), word[1..].iter().sum::<u64>()), (drawn_number, 0));
    }
}
