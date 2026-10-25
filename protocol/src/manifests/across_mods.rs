//! The mods' manifests checked together ([`check_across_mods`]), each
//! read and checked alone already: every mod and every bundle named
//! once among them all; every bundle depended on there, of a side its
//! dependent may depend on, and no ring of them; every type a type
//! names in its own bundle or in one its bundle depends on, of the kind
//! it must be; and no ring of guis nested in each other.

use super::{Bundle, Manifest, ManifestError, Side, Type};
use crate::kinds::Kind;
use crate::type_names::{TypeName, NAME_SEPARATOR};
use std::collections::HashMap;

/// Every bundle of the mods, by its name: its mod's manifest, and it.
type BundlesByName<'mods> = HashMap<&'mods str, (&'mods Manifest, &'mods Bundle)>;

/// `what` is wrong in `section` of `manifest`'s mod.
fn refused(manifest: &Manifest, section: &str, what: String) -> ManifestError {
    ManifestError { mod_folder: manifest.folder.clone(), line: None, section: section.to_string(), what }
}

/// `manifests` checked together: or the first thing wrong among them.
pub(super) fn check_across_mods(manifests: &[Manifest]) -> Result<(), ManifestError> {
    let mut bundles_by_name = BundlesByName::new();
    for (place, manifest) in manifests.iter().enumerate() {
        if let Some(named_before) = manifests[..place].iter().find(|read_before| read_before.name == manifest.name) {
            return Err(refused(manifest, &manifest.name, format!("a mod named as the one in {} is", named_before.folder.display())));
        }
        for bundle in &manifest.bundles {
            if let Some((other_mod, _)) = bundles_by_name.insert(&bundle.name, (manifest, bundle)) {
                return Err(refused(manifest, &bundle.name, format!("a bundle named as one of {}'s is", other_mod.name)));
            }
        }
    }
    for manifest in manifests {
        for bundle in &manifest.bundles {
            check_dependencies(manifest, bundle, &bundles_by_name)?;
            for bundle_type in &bundle.types {
                check_references(manifest, bundle, bundle_type, &bundles_by_name)?;
            }
        }
    }
    check_no_ring_of_dependencies(&bundles_by_name)?;
    check_no_ring_of_guis(&bundles_by_name)
}

/// Whether a bundle of the side `dependent` may depend on one of the
/// side `depended`: a common bundle on common ones alone, a server or a
/// client bundle on common ones and its side's.
fn may_depend(dependent: Side, depended: Side) -> bool {
    depended == Side::Common || depended == dependent
}

/// The bundles `bundle` depends on there, none itself, each of a side it
/// may depend on.
fn check_dependencies(manifest: &Manifest, bundle: &Bundle, bundles_by_name: &BundlesByName) -> Result<(), ManifestError> {
    for dependency in &bundle.depends_on {
        let Some(&(_, depended)) = bundles_by_name.get(dependency.as_str()) else {
            return Err(refused(manifest, &bundle.name, format!("depends on {dependency}: no mod has a bundle of that name")));
        };
        if depended.name == bundle.name {
            return Err(refused(manifest, &bundle.name, "depends on itself".to_string()));
        }
        if !may_depend(bundle.side, depended.side) {
            return Err(refused(manifest, &bundle.name, format!("a bundle of the {:?} side depends on {dependency}, of the {:?} side", bundle.side, depended.side)));
        }
    }
    Ok(())
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

/// The type `kind` names, if any: no kind names more than one.
fn reference(kind: &Kind) -> Option<Reference<'_>> {
    let (field, named, of_its_kind, kind_word): (_, _, fn(&Kind) -> bool, _) = match kind {
        Kind::Entity { default_attribute: Some(attribute) } => ("default attribute", attribute, |named| matches!(named, Kind::Attribute { .. }), "attribute"),
        Kind::EntityProc { entity } => ("entity", entity, |named| matches!(named, Kind::Entity { .. }), "entity"),
        Kind::Gui { parent: Some(parent) } => ("parent", parent, |named| matches!(named, Kind::Gui { .. }), "gui"),
        _ => return None,
    };
    Some(Reference { field, named, of_its_kind, kind_word })
}

/// The type `type_name` names, if any bundle has it.
fn type_named<'mods>(type_name: &TypeName, bundles_by_name: &BundlesByName<'mods>) -> Option<&'mods Type> {
    bundles_by_name.get(type_name.bundle.as_str())?.1.types.iter().find(|bundle_type| bundle_type.name == type_name.type_name)
}

/// The type `bundle_type` names, if any: in its own bundle or in one its
/// bundle depends on, there, and of the kind its field wants.
fn check_references(manifest: &Manifest, bundle: &Bundle, bundle_type: &Type, bundles_by_name: &BundlesByName) -> Result<(), ManifestError> {
    let section = format!("{}{NAME_SEPARATOR}{}", bundle.name, bundle_type.name);
    if let Some(Reference { field, named, of_its_kind, kind_word }) = reference(&bundle_type.kind) {
        let shown = format!("{}{NAME_SEPARATOR}{}", named.bundle, named.type_name);
        if named.bundle != bundle.name && !bundle.depends_on.contains(&named.bundle) {
            return Err(refused(manifest, &section, format!("{field}: {shown} is of a bundle {} does not depend on", bundle.name)));
        }
        match type_named(named, bundles_by_name) {
            None => return Err(refused(manifest, &section, format!("{field}: no type is {shown}"))),
            Some(found) if !of_its_kind(&found.kind) => return Err(refused(manifest, &section, format!("{field}: {shown} is no {kind_word}"))),
            Some(_) => {}
        }
    }
    Ok(())
}

/// No bundle depends on itself through others.
fn check_no_ring_of_dependencies(bundles_by_name: &BundlesByName) -> Result<(), ManifestError> {
    /// Where a bundle is in the walk: being walked from, or walked.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Walk {
        /// On the path walked now.
        OnThePath,
        /// Walked, no ring through it.
        Done,
    }
    /// Walks the bundles `bundle_name` depends on, and theirs: the ring
    /// found, if any, a bundle of it.
    fn walk<'mods>(bundle_name: &'mods str, bundles_by_name: &BundlesByName<'mods>, walks: &mut HashMap<&'mods str, Walk>) -> Option<&'mods str> {
        match walks.get(bundle_name) {
            Some(Walk::OnThePath) => return Some(bundle_name),
            Some(Walk::Done) => return None,
            None => {}
        }
        walks.insert(bundle_name, Walk::OnThePath);
        let (_, bundle) = bundles_by_name[bundle_name];
        for dependency in &bundle.depends_on {
            if let Some(in_the_ring) = walk(dependency, bundles_by_name, walks) {
                return Some(in_the_ring);
            }
        }
        walks.insert(bundle_name, Walk::Done);
        None
    }
    let mut walks = HashMap::new();
    for &bundle_name in bundles_by_name.keys() {
        if let Some(in_the_ring) = walk(bundle_name, bundles_by_name, &mut walks) {
            let (manifest, _) = bundles_by_name[in_the_ring];
            return Err(refused(manifest, in_the_ring, "depends on itself, through the bundles it depends on".to_string()));
        }
    }
    Ok(())
}

/// No gui nested in itself through its parents.
fn check_no_ring_of_guis(bundles_by_name: &BundlesByName) -> Result<(), ManifestError> {
    let type_count: usize = bundles_by_name.values().map(|(_, bundle)| bundle.types.len()).sum();
    for &(manifest, bundle) in bundles_by_name.values() {
        for gui in &bundle.types {
            let Kind::Gui { parent: mut next_parent } = gui.kind.clone() else { continue };
            let gui_name = TypeName { bundle: bundle.name.clone(), type_name: gui.name.clone() };
            // A gui's parents are no more than the types there are; a ring
            // above it, not through it, is found from a gui of that ring.
            for _ in 0..=type_count {
                let Some(parent) = next_parent else { break };
                if parent == gui_name {
                    return Err(refused(manifest, &format!("{}{NAME_SEPARATOR}{}", bundle.name, gui.name), "parent: nested in itself, through its parents".to_string()));
                }
                next_parent = type_named(&parent, bundles_by_name).and_then(|parent_type| match &parent_type.kind {
                    Kind::Gui { parent } => parent.clone(),
                    _ => None,
                });
            }
        }
    }
    Ok(())
}
