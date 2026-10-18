//! A type is registered once by its name, after what it names, and the
//! types frozen find each again -- tried on the base game's types.

use basegame::base_types::{CLIMATE, GRASS, GRASS_RULE, SHEEP, SHEEP_RULE, SHEEP_STATE, TREES_RULE, TREE_STAGE, WEATHER_RULE, SheepState};
use basegame::default_types::new_type_registry_with_default_types;
use protocol::type_registry::{data_words, Attribute, AttributeType, EntityRuleType, EntityType, LayerType, Layout, RegisteredType, RegistryRefusal, ScaRuleType, ATTRIBUTE_NUMBERS, TYPE_ID_NUMBERS};
use utilities::rng::RandomSource;

/// The first number no default type takes.
const FIRST_FREE_NUMBER: u64 = 100;

/// A number no default type takes, drawn: one any kind of type may have.
fn free_number(seeded_rng: &mut RandomSource) -> u64 {
    FIRST_FREE_NUMBER + seeded_rng.draw_below(TYPE_ID_NUMBERS.min(ATTRIBUTE_NUMBERS) - FIRST_FREE_NUMBER - 1)
}

/// The name of `registered`, if any: what tells a type from any other,
/// no two having one.
fn told_by(registered: Option<&RegisteredType>) -> Option<&str> {
    registered.map(RegisteredType::name)
}

/// Every type of `frozen_types` by its name, in their order.
fn each_told_by(frozen_types: &protocol::type_registry::FrozenTypes) -> Vec<&str> {
    frozen_types.types().iter().map(RegisteredType::name).collect()
}

/// A name is registered once: a type named as one registered before it
/// is refused, whatever the kind of either, and leaves the types as
/// they were; a name none has is registered.
#[test]
fn a_name_is_registered_once_whatever_its_kind() {
    let mut seeded_rng = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    let default_frozen = new_type_registry_with_default_types().freeze();
    for _ in 0..200 {
        let type_number = free_number(&mut seeded_rng);
        let of_a_drawn_kind = |name: &str, seeded_rng: &mut RandomSource| match seeded_rng.draw_below(3) {
            0 => RegisteredType::layer(name, LayerType::of_a_bit(type_number)),
            1 => RegisteredType::entity::<u64>(name, EntityType::from_number(type_number), None),
            _ => RegisteredType::attribute(name, Attribute::<u64>::new(type_number)),
        };
        let taken_name = default_frozen.types()[seeded_rng.draw_below(default_frozen.types().len() as u64) as usize].name();
        let mut type_registry = new_type_registry_with_default_types();
        assert_eq!(type_registry.register(of_a_drawn_kind(taken_name, &mut seeded_rng)), Err(RegistryRefusal::NameTaken), "{taken_name}");
        assert_eq!(each_told_by(&type_registry.clone().freeze()), each_told_by(&default_frozen), "a type refused is not kept");
        type_registry.register(of_a_drawn_kind("unheard of", &mut seeded_rng)).expect("a name no type has");
        assert_eq!(type_registry.freeze().types().len(), default_frozen.types().len() + 1);
    }
}

/// What a type names is registered before it: a default attribute, an
/// attribute of a fixed size; a rule's master, a plane for a rule of the
/// cells -- a layer or a wide plane -- and a type of entity for an
/// entity's. One named amiss -- nothing registered, or registered only
/// after, of another kind, an attribute whose size varies -- is refused.
#[test]
fn what_a_type_names_is_registered_before_it() {
    let mut seeded_rng = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    for _ in 0..100 {
        let type_number = free_number(&mut seeded_rng);
        let (fixed_attribute, varying_attribute) = (Attribute::<u64>::new(type_number), AttributeType::of_varying_size(type_number + 1));
        let entity_defaulting = |default_attribute| RegisteredType::Entity { name: "defaulting".to_string(), entity_type: EntityType::from_number(type_number), default_attribute: Some(default_attribute) };
        // The default types and an attribute whose size varies: each tried on them alone.
        let mut before = new_type_registry_with_default_types();
        let rule_of_cells = |master| RegisteredType::ScaRule { name: "a rule".to_string(), rule: ScaRuleType::from_number(type_number), master };
        let rule_of_entity = |master| RegisteredType::EntityRule { name: "a rule".to_string(), rule: EntityRuleType::from_number(type_number), master };
        before.register(RegisteredType::Attribute { name: "varying".to_string(), attribute_type: varying_attribute, data_words: None }).expect("a name no type has");
        let (default_amiss, master_amiss) = (Err(RegistryRefusal::DefaultAmiss), Err(RegistryRefusal::MasterAmiss));
        for (registered_type, outcome) in [
            (entity_defaulting(SHEEP_STATE.attribute_type()), Ok(())),
            (entity_defaulting(fixed_attribute.attribute_type()), default_amiss.clone()),
            (entity_defaulting(varying_attribute), default_amiss.clone()),
            (entity_defaulting(AttributeType(GRASS.0)), default_amiss),
            (rule_of_cells(GRASS), Ok(())),
            (rule_of_cells(CLIMATE.layer_type()), Ok(())),
            (rule_of_cells(LayerType::of_a_bit(type_number)), master_amiss.clone()),
            (rule_of_cells(LayerType::of_a_bit(SHEEP.type_number())), master_amiss.clone()),
            (rule_of_entity(SHEEP), Ok(())),
            (rule_of_entity(EntityType::from_number(type_number)), master_amiss.clone()),
            (rule_of_entity(EntityType::from_number(GRASS.type_number())), master_amiss),
        ] {
            assert_eq!(before.clone().register(registered_type.clone()), outcome, "{}", registered_type.name());
        }
        // Named before it is registered: refused, then registered once it is.
        let mut type_registry = before;
        let (entity, rule) = (entity_defaulting(fixed_attribute.attribute_type()), rule_of_entity(EntityType::from_number(type_number)));
        assert_eq!((type_registry.register(entity.clone()), type_registry.register(rule.clone())), (Err(RegistryRefusal::DefaultAmiss), Err(RegistryRefusal::MasterAmiss)));
        type_registry.register(RegisteredType::attribute("fixed", fixed_attribute)).expect("a name no type has");
        type_registry.register(entity).expect("its default registered before it");
        type_registry.register(rule).expect("its master registered before it");
    }
}

/// The types frozen find each again, by its name and by each of its
/// numbers; the layer types are every layer and wide plane; a sheep's
/// default attribute is its state, the only one; the rules tick in the
/// order registered, each among the rules of the master it names.
#[test]
fn the_frozen_types_find_each_type_again() {
    let frozen_types = new_type_registry_with_default_types().freeze();
    for registered_type in frozen_types.types() {
        assert_eq!(told_by(frozen_types.named(registered_type.name())), told_by(Some(registered_type)));
        assert!(registered_type.numbers().all(|type_number| told_by(frozen_types.of_number(type_number)) == told_by(Some(registered_type))), "{}", registered_type.name());
        assert_eq!(frozen_types.master_of(registered_type).is_some(), registered_type.is_a_rule(), "{}", registered_type.name());
    }
    assert!(frozen_types.named("unheard of").is_none());
    let layer_types = frozen_types.layer_types();
    assert_eq!(layer_types.len(), frozen_types.types().iter().filter(|registered_type| matches!(registered_type, RegisteredType::Layer { .. } | RegisteredType::WidePlane { .. })).count());
    assert!(layer_types.contains(&GRASS) && layer_types.contains(&TREE_STAGE.layer_type()) && layer_types.contains(&CLIMATE.layer_type()));
    assert_eq!(told_by(frozen_types.entity(SHEEP)), told_by(frozen_types.named("sheep")));
    assert_eq!(frozen_types.default_attributes(), [(SHEEP, SHEEP_STATE.attribute_type(), SheepState::WORDS)]);
    assert_eq!(frozen_types.rules().map(RegisteredType::name).collect::<Vec<_>>(), [GRASS_RULE, TREES_RULE, SHEEP_RULE, WEATHER_RULE]);
    for (rule_name, master_name) in [(GRASS_RULE, "grass"), (TREES_RULE, "tree"), (SHEEP_RULE, "sheep"), (WEATHER_RULE, "climate")] {
        let rule = frozen_types.named(rule_name).expect("registered");
        let master = frozen_types.master_of(rule).expect("its master registered");
        assert_eq!(master.name(), master_name);
        assert_eq!(frozen_types.rules_of(master).map(|master_rule| told_by(Some(master_rule))).collect::<Vec<_>>(), [told_by(Some(rule))], "{rule_name}");
    }
}

/// An ID says what it is of in its first byte and its size in its
/// last -- an attribute's blocks, a plane's bits a cell, nothing of an
/// entity type -- and keeps its number between; an attribute registered
/// says the words its data is; and a layout's fields come back from the
/// words they were written to.
#[test]
fn an_attribute_type_says_its_blocks_and_a_layout_its_fields() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = utilities::rng::RandomSource::new(seed);
    // The fewest blocks and the most, and every count between with a number drawn: any that fits between the two bytes.
    for block_count in 1..=protocol::type_registry::ATTRIBUTE_BLOCKS_MOST {
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
    let (state_type, frozen_types) = (SHEEP_STATE.attribute_type(), new_type_registry_with_default_types().freeze());
    assert_eq!((state_type.block_count(), frozen_types.data_words_of(state_type)), (3, Some(data_words(3))));
    assert_eq!(told_by(frozen_types.of_number(state_type.type_number())), told_by(frozen_types.named("sheep state")));
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
