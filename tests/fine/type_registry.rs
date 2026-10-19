//! A type is registered once by its name, after what it names, and the
//! types frozen find each again -- tried on the base game's types.

use basegame::base_types::{BaseTypes, GRASS_RULE, SHEEP_RULE, TREES_RULE, SheepState};
use protocol::type_registry::{AttributeType, Drawn, EntityType, FrozenTypeRegistry, LayerType, RegisteredType, Registration, RegistryRefusal, TypeKind, TypeRegistry, TYPE_ID_NUMBERS};
use server::simulation::simulation_api::{data_words, Attribute, Layout};
use utilities::rng::RandomSource;

/// A new world's registry with the base game's types registered, and
/// the IDs they were given.
fn with_base_types() -> (TypeRegistry, BaseTypes) {
    let mut type_registry = TypeRegistry::for_new_world();
    let base_types = basegame::default_types::register(&mut type_registry).expect("the base game's types registered");
    (type_registry, base_types)
}

/// The base game's types of a new world, frozen, and their IDs.
fn frozen_base_types() -> (FrozenTypeRegistry, BaseTypes) {
    let (type_registry, base_types) = with_base_types();
    (type_registry.freeze().expect("frozen"), base_types)
}

/// The type of `frozen_type_registry` named `type_name`, if any: a test's own
/// look-up, frozen types being known by their IDs alone.
fn named<'frozen>(frozen_type_registry: &'frozen FrozenTypeRegistry, type_name: &str) -> Option<&'frozen RegisteredType> {
    frozen_type_registry.types().iter().find(|registered_type| registered_type.name() == type_name)
}

/// The name of `registered`, if any: what tells a type from any other,
/// no two having one.
fn told_by(registered: Option<&RegisteredType>) -> Option<&str> {
    registered.map(RegisteredType::name)
}

/// Every type of `frozen_type_registry` by its name and ID, in their order.
fn each_told_by(frozen_type_registry: &FrozenTypeRegistry) -> Vec<(String, u64)> {
    frozen_type_registry.types().iter().map(|registered_type| (registered_type.name().to_string(), registered_type.type_id())).collect()
}

/// `registered`, its ID let go: whether a registration went through.
fn outcome(registered: Result<RegisteredType, RegistryRefusal>) -> Result<(), RegistryRefusal> {
    registered.map(drop)
}

/// A name is registered once: a type named as one registered before it
/// is refused, whatever the kind of either, and leaves the types as
/// they were; a name none has is registered, given the next ID.
#[test]
fn a_name_is_registered_once_whatever_its_kind() {
    let mut seeded_rng = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (default_frozen, _) = frozen_base_types();
    for _ in 0..200 {
        let of_a_drawn_kind = |name: &str, seeded_rng: &mut RandomSource| match seeded_rng.draw_below(5) {
            0 => Registration::layer(name),
            1 => Registration::entity(name, None),
            2 => Registration::render_rule(name, "grass"),
            3 => Registration::gui(name, None),
            _ => Attribute::<u64>::registration(name),
        };
        let taken_name = default_frozen.types()[seeded_rng.draw_below(default_frozen.types().len() as u64) as usize].name();
        let (mut type_registry, _) = with_base_types();
        assert_eq!(outcome(type_registry.register(of_a_drawn_kind(taken_name, &mut seeded_rng))), Err(RegistryRefusal::NameTaken), "{taken_name}");
        assert_eq!(each_told_by(&type_registry.clone().freeze().expect("frozen")), each_told_by(&default_frozen), "a type refused is not kept");
        let unheard_of = type_registry.register(of_a_drawn_kind("unheard of", &mut seeded_rng)).expect("a name no type has");
        assert!(default_frozen.types().iter().all(|registered_type| !registered_type.numbers().any(|number| unheard_of.numbers().contains(&number))), "an ID no type has");
        assert_eq!(type_registry.freeze().expect("frozen").types().len(), default_frozen.types().len() + 1);
    }
}

/// What a type names is registered before it: a default attribute, an
/// attribute; a rule's master, a plane for a rule of the cells -- a
/// layer or a wide plane -- and a type of entity for an entity's; a
/// rule of the renderer's, either; a window's parent, a window; a
/// client call's, any type; and each type it depends on. One named amiss -- nothing registered, or
/// registered only after, of another kind -- is refused.
#[test]
fn what_a_type_names_is_registered_before_it() {
    let (before, _) = with_base_types();
    let (default_amiss, master_amiss) = (Err(RegistryRefusal::DefaultAmiss), Err(RegistryRefusal::MasterAmiss));
    for (registration, wanted) in [
        (Registration::entity("defaulting", Some("sheep state")), Ok(())),
        (Registration::entity("defaulting", Some("fixed")), default_amiss.clone()),
        (Registration::entity("defaulting", Some("grass")), default_amiss),
        (Registration::sca_rule("a rule", "grass"), Ok(())),
        (Registration::sca_rule("a rule", "tree stage"), Ok(())),
        (Registration::sca_rule("a rule", "unheard of"), master_amiss.clone()),
        (Registration::sca_rule("a rule", "sheep"), master_amiss.clone()),
        (Registration::entity_rule("a rule", "sheep"), Ok(())),
        (Registration::entity_rule("a rule", "unheard of"), master_amiss.clone()),
        (Registration::entity_rule("a rule", "grass"), master_amiss.clone()),
        (Registration::render_rule("a rule", "grass"), Ok(())),
        (Registration::render_rule("a rule", "height"), Ok(())),
        (Registration::render_rule("a rule", "sheep"), Ok(())),
        (Registration::render_rule("a rule", "unheard of"), master_amiss.clone()),
        (Registration::render_rule("a rule", "sheep state"), master_amiss.clone()),
        (Registration::render_rule("a rule", GRASS_RULE), master_amiss),
        (Registration::gui("a window", None), Ok(())),
        (Registration::gui("a window", Some("unheard of")), Err(RegistryRefusal::ParentAmiss)),
        (Registration::gui("a window", Some("grass")), Err(RegistryRefusal::ParentAmiss)),
        (Registration::client_call("a call", "world generation"), Ok(())),
        (Registration::client_call("a call", "grass"), Ok(())),
        (Registration::client_call("a call", "unheard of"), Err(RegistryRefusal::MasterAmiss)),
        (Registration::layer("depending").depending_on(&["collision"]), Ok(())),
        (Registration::layer("depending").depending_on(&["unheard of"]), Err(RegistryRefusal::DependencyAmiss)),
    ] {
        let type_name = registration.name.clone();
        assert_eq!(outcome(before.clone().register(registration)), wanted, "{type_name}");
    }
    // Named before it is registered: refused, then registered once it is.
    let mut type_registry = before;
    let (entity, rule) = (Registration::entity("defaulting", Some("fixed")), Registration::entity_rule("a rule", "defaulting"));
    assert_eq!((outcome(type_registry.register(entity.clone())), outcome(type_registry.register(rule.clone()))), (Err(RegistryRefusal::DefaultAmiss), Err(RegistryRefusal::MasterAmiss)));
    type_registry.register(Attribute::<u64>::registration("fixed")).expect("a name no type has");
    type_registry.register(entity).expect("its default registered before it");
    type_registry.register(rule).expect("its master registered before it");
}

/// A new world's IDs are handed out as its types are registered, none
/// overlapping, the same every time; a saved world's types are each
/// given the ID the save gave its name, whatever order they come in, a
/// type the save does not name one past them all; a saved name given
/// to a type of another kind, or a plane of another shape, is refused,
/// and a saved name nobody registers keeps the types from freezing.
#[test]
fn a_saved_world_keeps_its_ids() {
    let (frozen_type_registry, base_types) = frozen_base_types();
    let numbers: Vec<_> = frozen_type_registry.types().iter().flat_map(RegisteredType::numbers).collect();
    assert!(numbers.windows(2).all(|pair| pair[0] < pair[1]) && numbers[0] >= 1, "handed out in order, none overlapping, none 0");
    assert_eq!(frozen_base_types().1, base_types, "the same IDs every time");
    let saved_text = frozen_type_registry.to_text();
    // Read back, in another order: each name its saved ID.
    let mut reordered = TypeRegistry::for_saved_world(&saved_text).expect("a world's types");
    let collision = reordered.register(Registration::layer("collision")).expect("saved");
    let grass = reordered.register(Registration::layer("grass")).expect("saved");
    let newcomer = reordered.register(Registration::layer("newcomer")).expect("a name the save does not hold");
    assert_eq!((collision.layer_type(), grass.layer_type()), (Some(base_types.collision), Some(base_types.grass)));
    assert!(newcomer.numbers().start > *numbers.last().expect("types"), "past every saved ID");
    assert!(reordered.freeze().is_err(), "saved types nobody registered");
    let mut as_another = TypeRegistry::for_saved_world(&saved_text).expect("a world's types");
    assert_eq!(outcome(as_another.register(Registration::entity("grass", None))), Err(RegistryRefusal::SavedAsAnother));
    // A plane's shape is no part of its ID: the save keeps it, and a plane of another width or tile size is refused.
    let mut reshaped = TypeRegistry::for_saved_world(&saved_text).expect("a world's types");
    assert_eq!(outcome(reshaped.register(Registration::wide_plane("grass", 2))), Err(RegistryRefusal::SavedAsAnother), "another width");
    assert_eq!(outcome(reshaped.register(Registration::wide_plane_of_tiles("grass", 1, 2))), Err(RegistryRefusal::SavedAsAnother), "another tile size");
    // Registered whole, the same types come back.
    let mut whole = TypeRegistry::for_saved_world(&saved_text).expect("a world's types");
    assert_eq!(basegame::default_types::register(&mut whole).expect("registered"), base_types);
    assert_eq!(each_told_by(&whole.freeze().expect("frozen")), each_told_by(&frozen_type_registry));
    assert!(TypeRegistry::for_saved_world("not,a world's types\n").is_err());
}

/// The types frozen find each again, by its name and by each of its
/// numbers; the layer types are every layer and wide plane; a sheep's
/// default attribute is its state, the only one; the rules tick in the
/// order registered, each among the rules of the master it names.
#[test]
fn the_frozen_types_find_each_type_again() {
    let (frozen_type_registry, base_types) = frozen_base_types();
    for registered_type in frozen_type_registry.types() {
        assert!(registered_type.numbers().all(|type_number| told_by(frozen_type_registry.of_number(type_number)) == told_by(Some(registered_type))), "{}", registered_type.name());
        assert_eq!(frozen_type_registry.master_of(registered_type).is_some(), registered_type.is_a_rule(), "{}", registered_type.name());
    }
    let layer_types = frozen_type_registry.layer_types();
    assert_eq!(layer_types.len(), frozen_type_registry.types().iter().filter(|registered_type| matches!(registered_type, RegisteredType::Layer { .. } | RegisteredType::WidePlane { .. })).count());
    assert!(layer_types.contains(&base_types.grass) && layer_types.contains(&base_types.tree_stage.layer_type()));
    assert_eq!(told_by(frozen_type_registry.entity(base_types.sheep)), Some("sheep"));
    assert_eq!(frozen_type_registry.default_attributes(), [(base_types.sheep, base_types.sheep_state.attribute_type(), SheepState::WORDS)]);
    assert_eq!(frozen_type_registry.rules().map(RegisteredType::name).collect::<Vec<_>>(), [GRASS_RULE, TREES_RULE, SHEEP_RULE]);
    for (rule_name, master_name) in [(GRASS_RULE, "grass"), (TREES_RULE, "tree"), (SHEEP_RULE, "sheep")] {
        let rule = named(&frozen_type_registry, rule_name).expect("registered");
        let master = frozen_type_registry.master_of(rule).expect("its master registered");
        assert_eq!(master.name(), master_name);
        assert_eq!(frozen_type_registry.rules_of(master).map(|master_rule| told_by(Some(master_rule))).collect::<Vec<_>>(), [told_by(Some(rule))], "{rule_name}");
    }
}

/// An ID says what it is of in its first byte and keeps its number
/// under it -- an attribute's last four bits its blocks, as yet; a
/// plane's bits a cell beside its ID, not in it; an attribute registered
/// says the words its data is; and a layout's fields come back from the
/// words they were written to.
#[test]
fn an_attribute_type_says_its_blocks_and_a_layout_its_fields() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_rng = utilities::rng::RandomSource::new(seed);
    // The fewest blocks and the most, and every count between with a number drawn: any an attribute's ID holds.
    for block_count in 1..=AttributeType::MOST_BLOCKS {
        let type_number = 1 + seeded_rng.draw_below(protocol::type_registry::ATTRIBUTE_NUMBERS - 1);
        let drawn_type = AttributeType::of_blocks(type_number, block_count);
        assert_eq!((drawn_type.block_count(), drawn_type.type_number(), drawn_type.0 & 0xF, drawn_type.0 >> 56, drawn_type.is_an_attributes()), (block_count, type_number, block_count as u64 - 1, 0x00, true));
        let varying_type = AttributeType::of_varying_size(type_number);
        // One whose size varies is registered with its four bits 0; an attribute of it so many blocks long says so there, and is of the same type.
        assert_eq!((varying_type.0 & 0xF, varying_type.type_number(), varying_type.is_an_attributes()), (0, type_number, true));
        assert_eq!((varying_type.with_blocks(block_count).block_count(), varying_type.with_blocks(block_count).type_identity()), (block_count, varying_type.type_identity()));
        let (drawn_width, first_number) = (1u32 << seeded_rng.draw_below(5), seeded_rng.draw_below(TYPE_ID_NUMBERS - 16));
        let wide_layer = LayerType::of_width(first_number, drawn_width);
        assert_eq!((wide_layer.width_bits(), wide_layer.type_number(), wide_layer.type_id()), (drawn_width, first_number, 0x01 << 56 | first_number), "the ID its kind and number alone, the width beside it");
        // A plane a bit a cell is its own bitplane; a wide one's bitplanes take the numbers after its own, no two IDs one.
        let first_bitplane = first_number + u64::from(drawn_width > 1);
        assert!((0..drawn_width).all(|bit_index| wide_layer.holds_bitplane(wide_layer.bitplane_of(bit_index)) && wide_layer.bitplane_of(bit_index) == LayerType::of_a_bit(first_bitplane + u64::from(bit_index))) && !wide_layer.holds_bitplane(wide_layer.bitplane_of(drawn_width)));
        assert!(drawn_width == 1 || wide_layer.each_bitplane().all(|bitplane| bitplane.type_id() != wide_layer.type_id()), "a wide plane's ID none of its bitplanes'");
        let entity = EntityType::from_number(type_number);
        assert_eq!((entity.type_number(), entity.0), (type_number, 0x02 << 56 | type_number));
    }
    // An ID of another kind is no attribute's: the one namespace is everything's.
    for other_id in [frozen_base_types().1.grass.type_id(), frozen_base_types().1.sheep.0, frozen_base_types().1.tree_stage.layer_type().type_id(), 0] {
        assert!(!AttributeType(other_id).is_an_attributes());
    }
    let (frozen_type_registry, base_types) = frozen_base_types();
    let state_type = base_types.sheep_state.attribute_type();
    assert_eq!((state_type.block_count(), frozen_type_registry.data_words_of(state_type)), (3, Some(data_words(3))));
    assert_eq!(told_by(frozen_type_registry.of_number(state_type.type_number())), Some("sheep state"));
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

/// A rule of the renderer is of its own kind, and says what it draws:
/// the plane or the type of entity its master is. A window nested in
/// another holds its parent's ID -- a child names its parent, once the
/// parent is registered -- and a window on its own, none. A function a
/// client may call is of its own kind, holding its master's ID.
#[test]
fn a_rule_of_the_renderer_draws_its_master_and_a_window_holds_its_parent() {
    let (mut type_registry, base_types) = with_base_types();
    let grass_drawn = type_registry.register(Registration::render_rule("grass drawn", "grass")).expect("of a plane");
    let sheep_drawn = type_registry.register(Registration::render_rule("sheep drawn", "sheep")).expect("of a type of entity");
    assert!(matches!(grass_drawn, RegisteredType::RenderRule { master: Drawn::Plane(layer_type), .. } if layer_type == base_types.grass));
    assert!(matches!(sheep_drawn, RegisteredType::RenderRule { master: Drawn::Entity(entity_type), .. } if entity_type == base_types.sheep));
    assert!([grass_drawn.type_id(), sheep_drawn.type_id()].iter().all(|type_id| type_id >> 56 == TypeKind::RenderRule as u64), "of the renderer's kind");
    let menu = type_registry.register(Registration::gui("menu", None)).expect("on its own");
    let entry = type_registry.register(Registration::gui("entry", Some("menu"))).expect("its parent registered before it");
    let (RegisteredType::Gui { gui: menu_id, parent: None, .. }, RegisteredType::Gui { parent: Some(entry_parent), .. }) = (&menu, &entry) else { panic!("windows: {menu:?}, {entry:?}") };
    assert_eq!(entry_parent, menu_id, "the child holds its parent's ID");
    assert_eq!(menu.type_id() >> 56, TypeKind::Gui as u64);
    let call = type_registry.register(Registration::client_call("a call", "world generation")).expect("of a type registered before it");
    let world_generation = type_registry.clone().freeze().expect("frozen").types().iter().find(|registered| registered.name() == "world generation").expect("registered").type_id();
    assert!(matches!(call, RegisteredType::ClientCall { master, .. } if master == world_generation), "the call holds its master's ID");
    assert_eq!(call.type_id() >> 56, TypeKind::ClientCall as u64);
    assert!(!grass_drawn.is_a_rule() && !menu.is_a_rule() && !call.is_a_rule(), "nothing the simulation ticks");
    assert_eq!(type_registry.freeze().expect("frozen").rules().count(), 3, "the base game's three rules tick, and no rule of the renderer");
}
