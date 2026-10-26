//! A smoke test: three mods' manifests, a container among them carrying
//! one, made into the registry's flat list.

use protocol::kinds::{Kind, ModFields, Side};
use protocol::manifests::read_mods;
use protocol::packages::{DEPENDENCIES_FOLDER, MANIFEST_FILE, PACKAGE_EXTENSION};
use protocol::type_registry::{TypeRegistry, KIND_SHIFT};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use utilities::archive::write_archive;
use utilities::dynamic_library::library_file_name;

/// The package of the mod `mod_name`: its manifest, `{hash}` in it the
/// library's hash; its library, holding `library_bytes`, if it has one;
/// and the packages `carried`, each by its mod's name.
fn package_of(mod_name: &str, library_bytes: Option<&[u8]>, manifest: &str, carried: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let manifest = library_bytes.map_or(manifest.to_string(), |library_bytes| manifest.replace("{hash}", &format!("0x{:x}", Sha256::digest(library_bytes))));
    let library_file = library_file_name(mod_name);
    let carried_paths: Vec<String> = carried.iter().map(|(carried_name, _)| format!("{DEPENDENCIES_FOLDER}{carried_name}{PACKAGE_EXTENSION}")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![(MANIFEST_FILE, manifest.as_bytes())];
    entries.extend(library_bytes.map(|library_bytes| (library_file.as_str(), library_bytes)));
    entries.extend(carried_paths.iter().zip(carried).map(|(path, (_, package))| (path.as_str(), package.as_slice())));
    write_archive(&entries)
}

/// The package `package` written in `test_folder` as `file_name`: where.
fn written(test_folder: &Path, file_name: &str, package: &[u8]) -> PathBuf {
    std::fs::create_dir_all(test_folder).expect("the test's folder");
    let path = test_folder.join(format!("{file_name}{PACKAGE_EXTENSION}"));
    std::fs::write(&path, package).expect("the package written");
    path
}

/// The package of the mod `mod_name`, carrying none, written in
/// `test_folder`: where.
fn mod_made(test_folder: &Path, mod_name: &str, library_bytes: Option<&[u8]>, manifest: &str) -> PathBuf {
    written(test_folder, mod_name, &package_of(mod_name, library_bytes, manifest, &[]))
}

/// Every mod and type, in order, of its full name and of an ID counting
/// up whatever its kind, its kind's byte first.
#[test]
fn the_manifests_make_one_flat_list() {
    let test_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("registry_smoke");
    _ = std::fs::remove_dir_all(&test_folder);
    let base = mod_made(&test_folder, "base", Some(b"base bytes"), "\
:base:
- side : common
- version : 0.1
- hash : {hash}

grass :
- kind : plane
- width : 1
- tile size : 1
");
    let other = package_of("other", Some(b"other bytes"), "\
:other:
- side : common
- version : 3
- hash : {hash}
- depends on :
    -- base : 0.1

water :
- kind : plane
- width : 4
- tile size : 2
", &[]);
    let pack = written(&test_folder, "pack", &package_of("pack", None, "\
:pack:
- side : common
- version : 1
- container mod
- depends on :
    - base : 0.1
    -- other : 3
", &[("other", other)]));
    let registry = TypeRegistry::of_manifests(&read_mods(&[base, pack]).expect("read and checked"));
    let listed: Vec<(&str, u64)> = registry.types().iter().map(|registered| (registered.full_name(), registered.type_id())).collect();
    let id = |kind_byte: u64, number: u64| kind_byte << KIND_SHIFT | number;
    assert_eq!(listed, [("base", id(0x09, 0)), ("base>grass", id(0x01, 1)), ("pack", id(0x09, 2)), ("other", id(0x09, 3)), ("other>water", id(0x01, 4))]);
    assert_eq!(registry.types()[4].kind(), &Kind::Plane { width_bits: 4, tile_size: 2 });
    let Kind::Mod(ModFields { side, depends_on, .. }) = registry.types()[3].kind() else { panic!("a mod") };
    assert_eq!((*side, &depends_on[..]), (Side::Common, &[("base".to_string(), "0.1".to_string())][..]));
    let Kind::Mod(ModFields { library, .. }) = registry.types()[2].kind() else { panic!("a mod") };
    assert!(library.is_none(), "a container's library is none");
    _ = std::fs::remove_dir_all(&test_folder);
}
