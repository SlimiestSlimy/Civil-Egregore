//! Mods' manifests: one of every kind registers, and writes back as it
//! was read; a bundle names the types of the bundles it depends on alone;
//! and each thing a manifest can have wrong is refused at its row and
//! column.

use protocol::type_registry::{manifest_text, parse_manifest, BundleSide, Hook, PlaneTileSize, PlaneWidth, RegisteredType, TypeRegistry};

/// A mod of every kind, its columns in an order of their own: three
/// bundles, on the server and the client, two depending on the first;
/// hooks; an entity with its default attribute and its rule; a window in
/// a window.
const EVERY_KIND: &str = "\
kind,type,width,tile size,hooks,default attribute,rule,blocks,data words,draw order,parent,depends on,side
bundle,tests,,,,,,,,,,,
bundle,cells,,,,,,,,,,,server
plane,grass,1,1,,,,,,,,,
plane,temperature,8,16,,,,,,,,,
sca rule,grass rule,,,,,,,,,,,
end,cells,,,,,,,,,,,
bundle,animals,,,,,,,,,,cells,server
attribute,sheep state,,,,,,2,3,,,,
entity rule,sheep rule,,,,,,,,,,,
entity,sheep,,,keeps the world hot,sheep state,sheep rule,,,,,,
world generation,world generation,,,generates the world,,,,,,,,
end,animals,,,,,,,,,,,
bundle,client,,,,,,,,,,cells,client
render rule,grass drawn,,,,,,,,5,,,
gui,menu,,,shown by default,,,,,,,,
gui,menu settings,,,,,,,,,menu,,
client call,ask the time,,,,,,,,,,,
end,client,,,,,,,,,,,
end,tests,,,,,,,,,,,
";

/// The type of the whole name `name` among `types`.
fn registered<'registry>(types: &'registry [RegisteredType], name: &str) -> &'registry RegisteredType {
    types.iter().find(|registered| registered.name() == name).unwrap_or_else(|| panic!("`{name}` registered"))
}

/// A manifest of every kind writes back as it was read, and registers:
/// whole names, planes' shapes, the entity's default attribute and rule,
/// the window's parent, the bundles' sides and dependencies, and the
/// hooks.
#[test]
fn a_manifest_of_every_kind_registers_and_writes_back_as_read() {
    let manifest_rows = parse_manifest(EVERY_KIND).expect("a manifest of every kind");
    assert_eq!(parse_manifest(&manifest_text(&manifest_rows)), Ok(manifest_rows), "written back as read");
    let mut type_registry = TypeRegistry::for_new_world();
    type_registry.register_manifest(EVERY_KIND).expect("registered");
    let frozen = type_registry.freeze().expect("frozen");
    let types = frozen.types();
    assert_eq!(types.len(), 15, "the mod's bundle, its three, and their types");
    let temperature = registered(types, "tests::cells::temperature").plane_type().expect("a plane");
    assert_eq!((temperature.width(), temperature.tile_size()), (PlaneWidth::Bits8, PlaneTileSize::Side16));
    let RegisteredType::Entity { default_attribute, rule, .. } = registered(types, "tests::animals::sheep") else { panic!("an entity") };
    assert_eq!(*default_attribute, registered(types, "tests::animals::sheep state").attribute_type(), "its default attribute");
    assert_eq!(rule.map(|rule| rule.0), Some(registered(types, "tests::animals::sheep rule").type_id()), "its rule");
    let RegisteredType::Gui { parent, .. } = registered(types, "tests::client::menu settings") else { panic!("a window") };
    assert_eq!(parent.map(|parent| parent.0), Some(registered(types, "tests::client::menu").type_id()));
    let RegisteredType::Bundle { side, depends_on, .. } = registered(types, "tests::animals") else { panic!("a bundle") };
    assert_eq!((*side, depends_on.iter().map(|bundle| bundle.0).collect::<Vec<_>>()), (Some(BundleSide::Server), vec![registered(types, "tests::cells").type_id()]));
    assert!(matches!(registered(types, "tests::client"), RegisteredType::Bundle { side: Some(BundleSide::Client), .. }));
    assert!(matches!(registered(types, "tests"), RegisteredType::Bundle { side: None, .. }), "the mod's own on no side");
    assert_eq!(frozen.hooked(Hook::KeepsTheWorldHot).map(RegisteredType::name).collect::<Vec<_>>(), ["tests::animals::sheep"]);
    assert_eq!(frozen.hooked(Hook::GeneratesTheWorld).map(RegisteredType::name).collect::<Vec<_>>(), ["tests::animals::world generation"]);
}

/// A bundle names the types of the bundles it depends on, written whole
/// -- of its own mod or another -- and of no other.
#[test]
fn a_bundle_names_the_types_of_the_bundles_it_depends_on_alone() {
    let mut type_registry = TypeRegistry::for_new_world();
    type_registry.register_manifest(EVERY_KIND).expect("the first mod");
    let depending = "type,kind,side,depends on,parent\nmore,bundle,,,\nwindows,bundle,client,tests::client,\nmore settings,gui,,,tests::client::menu\nwindows,end,,,\nmore,end,,,\n";
    type_registry.register_manifest(depending).expect("a bundle naming a type of one it depends on");
    let apart = "type,kind,side,parent\napart,bundle,,\nwindows,bundle,client,\napart settings,gui,,tests::client::menu\nwindows,end,,\napart,end,,\n";
    let refused = type_registry.register_manifest(apart).expect_err("a bundle naming a type of one it does not depend on");
    assert_eq!((refused.row, refused.column), (3, "type"));
    assert!(refused.what.contains("OutsideItsDependencies"), "{refused}");
}

/// Each thing a manifest can have wrong is refused, at its row and column.
#[test]
fn what_a_manifest_has_wrong_is_refused_where_it_is() {
    let refusals: [(&str, &str, usize, &str); 22] = [
        ("a column no manifest has", "type,kind,colour\ntests,bundle,red\n", 0, "columns"),
        ("no kind column", "type\ntests\n", 0, "columns"),
        ("no row", "type,kind\n", 1, "type"),
        ("a first row of no bundle", "type,kind,width,tile size\ngrass,plane,1,1\n", 1, "type"),
        ("the mod's bundle depending", "type,kind,depends on\ntests,bundle,other\n", 1, "depends on"),
        ("the mod's bundle on a side", "type,kind,side\ntests,bundle,server\n", 1, "side"),
        ("a bundle of the mod on no side", "type,kind,side\ntests,bundle,\ncells,bundle,\n", 2, "side"),
        ("a side there is not", "type,kind,side\ntests,bundle,\ncells,bundle,both\n", 2, "side"),
        ("a side on a type", "type,kind,side\ntests,bundle,\ncells,bundle,server\ngrass rule,sca rule,server\n", 3, "side"),
        ("a type in the mod's own bundle", "type,kind\ntests,bundle\ngrass rule,sca rule\n", 2, "type"),
        ("a bundle in a bundle of the mod", "type,kind,side\ntests,bundle,\ncells,bundle,server\ndeeper,bundle,server\n", 3, "type"),
        ("a kind no type has", "type,kind,side\ntests,bundle,\ncells,bundle,server\ngrass,lawn,\n", 3, "kind"),
        ("a hook there is not", "type,kind,side,hooks\ntests,bundle,,\ncells,bundle,server,\ngrass rule,sca rule,,runs every tick\n", 3, "hooks"),
        ("a width no plane has", "type,kind,side,width,tile size\ntests,bundle,,,\ncells,bundle,server,,\ngrass,plane,,3,1\n", 3, "width"),
        ("a tile size no plane has", "type,kind,side,width,tile size\ntests,bundle,,,\ncells,bundle,server,,\ngrass,plane,,1,512\n", 3, "tile size"),
        ("a field its kind has none of", "type,kind,side,width\ntests,bundle,,\nanimals,bundle,server,\nsheep,entity,,4\n", 3, "width"),
        ("a field missing", "type,kind,side,blocks,data words\ntests,bundle,,,\nanimals,bundle,server,,\nstate,attribute,,2,\n", 3, "data words"),
        ("an end of a bundle not open", "type,kind,side\ntests,bundle,\ncells,bundle,server\ntests,end,\n", 3, "type"),
        ("a bundle never ended", "type,kind,side\ntests,bundle,\ncells,bundle,server\ncells,end,\n", 3, "type"),
        ("a row past the mod's end", "type,kind\ntests,bundle\ntests,end\nmore,bundle\n", 3, "type"),
        ("a dependency on no bundle", "type,kind,side,depends on\ntests,bundle,,\ncells,bundle,server,nowhere\n", 2, "type"),
        ("a default attribute of no attribute", "type,kind,side,width,tile size,default attribute\ntests,bundle,,,,\ncells,bundle,server,,,\ngrass,plane,,1,1,\nsheep,entity,,,,grass\ncells,end,,,,\ntests,end,,,,\n", 4, "type"),
    ];
    for (wrong, manifest, row, column) in refusals {
        let refused = TypeRegistry::for_new_world().register_manifest(manifest).expect_err(wrong);
        assert_eq!((refused.row, refused.column), (row, column), "{wrong}: {refused}");
    }
}
