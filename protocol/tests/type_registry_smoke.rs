//! A smoke test: two mods' manifests made into the registry's flat list.

use protocol::kinds::{BundleFields, Kind, Side};
use protocol::manifests::{read_mods, MANIFEST_FILE};
use protocol::type_registry::{TypeRegistry, KIND_SHIFT};
use utilities::hash::hash_bytes;
use std::path::{Path, PathBuf};

/// A mod's folder made in `test_folder`: its library `library_name`,
/// holding `library_bytes`, and its manifest, `{hash}` in it the
/// library's hash.
fn mod_made(test_folder: &Path, mod_name: &str, library_name: &str, library_bytes: &[u8], manifest: &str) -> PathBuf {
    let mod_folder = test_folder.join(mod_name);
    std::fs::create_dir_all(&mod_folder).expect("the mod's folder");
    std::fs::write(mod_folder.join(library_name), library_bytes).expect("the library written");
    std::fs::write(mod_folder.join(MANIFEST_FILE), manifest.replace("{hash}", &format!("{:016x}", hash_bytes(library_bytes)))).expect("the manifest written");
    mod_folder
}

/// Every bundle and type, in order, of its full name and of an ID
/// counting up whatever its kind, its kind's byte first.
#[test]
fn the_manifests_make_one_flat_list() {
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("registry_smoke");
    _ = std::fs::remove_dir_all(&test_folder);
    let base = mod_made(&test_folder, "base", "libbase.so", b"base bytes", "\
:base:

::planes::
- side : common
- library : libbase.so
- hash : {hash}

grass :
- kind : plane
- width : 1
- tile size : 1
");
    let other = mod_made(&test_folder, "other", "libother.so", b"other bytes", "\
:other:

::planes::
- side : common
- library : libother.so
- hash : {hash}
- depends on :
- base>planes


water :
- kind : plane
- width : 4
- tile size : 2
");
    let registry = TypeRegistry::of_manifests(&read_mods(&[base, other]).expect("read and checked"));
    let listed: Vec<(&str, u64)> = registry.types().iter().map(|registered| (registered.full_name(), registered.type_id())).collect();
    let id = |kind_byte: u64, number: u64| kind_byte << KIND_SHIFT | number;
    assert_eq!(listed, [("base>planes", id(0x09, 0)), ("base>planes>grass", id(0x01, 1)), ("other>planes", id(0x09, 2)), ("other>planes>water", id(0x01, 3))]);
    assert_eq!(registry.types()[3].kind(), &Kind::Plane { width_bits: 4, tile_size: 2 });
    let Kind::Bundle(BundleFields { side, depends_on, .. }) = registry.types()[2].kind() else { panic!("a bundle") };
    assert_eq!((*side, depends_on[0].to_string()), (Side::Common, "base>planes".to_string()));
    _ = std::fs::remove_dir_all(&test_folder);
}
