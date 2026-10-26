//! The mods' manifests checked together ([`check_across_mods`]), each
//! read and checked alone already, each named once among them all:
//! every mod depended on there, at the version depended on, of a side
//! its dependent may depend on -- a common container on any -- and no
//! ring of them; every type a type names in its own mod or in one it reaches, of
//! the kind it must be; and no ring of guis nested in each other.

use super::{Manifest, ManifestError};
use crate::kinds::{Kind, Side};
use crate::manifest_format::Type;
use crate::type_names::TypeName;
use std::collections::HashMap;

/// Every mod's manifest, by the mod's name.
type ModsByName<'mods> = HashMap<&'mods str, &'mods Manifest>;

/// `what` is wrong in `section` of `manifest`'s mod.
fn refused(manifest: &Manifest, section: &str, what: String) -> ManifestError {
    ManifestError { package: manifest.package.clone(), line: None, section: section.to_string(), what }
}

/// `manifests` checked together: or the first thing wrong among them, in
/// the order given.
pub(super) fn check_across_mods(manifests: &[Manifest]) -> Result<(), ManifestError> {
    let mods_by_name: ModsByName = manifests.iter().map(|manifest| (manifest.document.name.as_str(), manifest)).collect();
    for manifest in manifests {
        check_dependencies(manifest, &mods_by_name)?;
        let reach = reach_of(manifest, &mods_by_name);
        for each_type in &manifest.document.types {
            check_references(manifest, each_type, &reach, &mods_by_name)?;
        }
    }
    check_no_ring_of_dependencies(manifests, &mods_by_name)?;
    check_no_ring_of_guis(manifests, &mods_by_name)
}

/// Whether a mod of the side `dependent` may depend on one of the side
/// `depended`: a common mod on common ones alone, a server or a client
/// mod on common ones and its side's.
fn may_depend(dependent: Side, depended: Side) -> bool {
    depended == Side::Common || depended == dependent
}

/// Whether a container of the side `container` may carry a mod of the
/// side `carried`: a common one any, as both sides take it; another as
/// a mod of its side may depend on it.
fn may_carry(container: Side, carried: Side) -> bool {
    container == Side::Common || may_depend(container, carried)
}

/// Whether `manifest`'s mod is a container: no library.
fn is_a_container(manifest: &Manifest) -> bool {
    manifest.document.container
}

/// The mods `manifest`'s depends on there, each at the version said, of
/// a side it may depend on, or carry if it is a container.
fn check_dependencies(manifest: &Manifest, mods_by_name: &ModsByName) -> Result<(), ManifestError> {
    for (dependency, version) in &manifest.document.depends_on {
        let Some(depended) = mods_by_name.get(dependency.as_str()) else {
            return Err(refused(manifest, &manifest.document.name, format!("depends on {dependency}: no such mod")));
        };
        if depended.document.version != *version {
            return Err(refused(manifest, &manifest.document.name, format!("depends on {dependency} {version}: {dependency} is {}", depended.document.version)));
        }
        let allowed = if is_a_container(manifest) { may_carry } else { may_depend };
        if !allowed(manifest.document.side, depended.document.side) {
            return Err(refused(manifest, &manifest.document.name, format!("a mod of the {:?} side depends on {dependency}, of the {:?} side", manifest.document.side, depended.document.side)));
        }
    }
    Ok(())
}

/// The mods whose types `manifest`'s may name: its own, those it depends
/// on, and what the containers among them carry, through containers
/// alone, each of a side it may depend on.
fn reach_of<'mods>(manifest: &'mods Manifest, mods_by_name: &ModsByName<'mods>) -> Vec<&'mods str> {
    let mut reach = vec![manifest.document.name.as_str()];
    let mut containers_to_open = vec![manifest];
    while let Some(container) = containers_to_open.pop() {
        for (dependency, _) in &container.document.depends_on {
            let Some(&depended) = mods_by_name.get(dependency.as_str()) else { continue };
            if reach.contains(&dependency.as_str()) || !may_depend(manifest.document.side, depended.document.side) {
                continue;
            }
            reach.push(&depended.document.name);
            if is_a_container(depended) {
                containers_to_open.push(depended);
            }
        }
    }
    reach
}

/// A type a type names: in which field, and the kind it must be.
struct Reference<'kind> {
    /// The field naming it.
    field: &'static str,
    /// The type named.
    named: &'kind TypeName,
    /// Whether a kind is the one it must be.
    of_its_kind: fn(&Kind) -> bool,
    /// That kind's word.
    kind_word: &'static str,
}

/// The types `kind` names: one field's, as many as it names.
fn references(kind: &Kind) -> Vec<Reference<'_>> {
    let (field, named, of_its_kind, kind_word): (_, Vec<&TypeName>, fn(&Kind) -> bool, _) = match kind {
        Kind::Entity { default_attribute: Some(attribute) } => ("default attribute", vec![attribute], |named| matches!(named, Kind::Attribute { .. }), "attribute"),
        Kind::EntityProc { entity } => ("entity", vec![entity], |named| matches!(named, Kind::Entity { .. }), "entity"),
        Kind::ScaProc { planes, .. } => ("planes", planes.iter().collect(), |named| matches!(named, Kind::Plane { .. }), "plane"),
        Kind::Gui { parent: Some(parent) } => ("parent", vec![parent], |named| matches!(named, Kind::Gui { .. }), "gui"),
        _ => return Vec::new(),
    };
    named.into_iter().map(|named| Reference { field, named, of_its_kind, kind_word }).collect()
}

/// The type `type_name` names, if its mod has it.
fn type_named<'mods>(type_name: &TypeName, mods_by_name: &ModsByName<'mods>) -> Option<&'mods Type> {
    mods_by_name.get(type_name.mod_name.as_str())?.document.types.iter().find(|each_type| each_type.name() == type_name.type_name)
}

/// The types `named_by` names, if any: each of a mod in `reach`, there,
/// and of the kind its field wants.
fn check_references(manifest: &Manifest, named_by: &Type, reach: &[&str], mods_by_name: &ModsByName) -> Result<(), ManifestError> {
    let section = TypeName { mod_name: manifest.document.name.clone(), type_name: named_by.name().to_string() }.to_string();
    for Reference { field, named, of_its_kind, kind_word } in references(named_by.kind()) {
        if !reach.contains(&named.mod_name.as_str()) {
            return Err(refused(manifest, &section, format!("{field}: {named} is of a mod {} does not reach -- one it depends on, or one a container it depends on carries, of a side it may depend on", manifest.document.name)));
        }
        match type_named(named, mods_by_name) {
            None => return Err(refused(manifest, &section, format!("{field}: no type is {named}"))),
            Some(found) if !of_its_kind(found.kind()) => return Err(refused(manifest, &section, format!("{field}: {named} is no {kind_word}"))),
            Some(_) => {}
        }
    }
    Ok(())
}

/// No mod depends on itself through others.
fn check_no_ring_of_dependencies(manifests: &[Manifest], mods_by_name: &ModsByName) -> Result<(), ManifestError> {
    /// Where a mod is in the walk.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Walk {
        /// On the path walked now.
        OnThePath,
        /// Walked, no ring through it.
        Done,
    }
    /// Walks the mods `manifest`'s depends on, and theirs: a mod of the
    /// ring found, if any.
    fn walk<'mods>(manifest: &'mods Manifest, mods_by_name: &ModsByName<'mods>, walks: &mut HashMap<&'mods str, Walk>) -> Option<&'mods Manifest> {
        match walks.get(manifest.document.name.as_str()) {
            Some(Walk::OnThePath) => return Some(manifest),
            Some(Walk::Done) => return None,
            None => {}
        }
        walks.insert(&manifest.document.name, Walk::OnThePath);
        for (dependency, _) in &manifest.document.depends_on {
            if let Some(in_the_ring) = walk(mods_by_name[dependency.as_str()], mods_by_name, walks) {
                return Some(in_the_ring);
            }
        }
        walks.insert(&manifest.document.name, Walk::Done);
        None
    }
    let mut walks = HashMap::new();
    for manifest in manifests {
        if let Some(in_the_ring) = walk(manifest, mods_by_name, &mut walks) {
            return Err(refused(in_the_ring, &in_the_ring.document.name, "depends on itself, through the mods it depends on".to_string()));
        }
    }
    Ok(())
}

/// No gui nested in itself through its parents.
fn check_no_ring_of_guis(manifests: &[Manifest], mods_by_name: &ModsByName) -> Result<(), ManifestError> {
    let type_count: usize = manifests.iter().map(|manifest| manifest.document.types.len()).sum();
    for manifest in manifests {
        for gui in &manifest.document.types {
            let Kind::Gui { parent: mut next_parent } = gui.kind().clone() else { continue };
            let gui_name = TypeName { mod_name: manifest.document.name.clone(), type_name: gui.name().to_string() };
            // A gui's parents are no more than the types there are; a ring
            // above it, not through it, is found from a gui of that ring.
            for _ in 0..=type_count {
                let Some(parent) = next_parent else { break };
                if parent == gui_name {
                    return Err(refused(manifest, &gui_name.to_string(), "parent: nested in itself, through its parents".to_string()));
                }
                next_parent = type_named(&parent, mods_by_name).and_then(|parent_type| match parent_type.kind() {
                    Kind::Gui { parent } => parent.clone(),
                    _ => None,
                });
            }
        }
    }
    Ok(())
}
