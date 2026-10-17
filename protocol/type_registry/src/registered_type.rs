//! A type a world has: a name, and what goes with it by its kind. What
//! runs it, if anything does, is its library, hooked in by its
//! registration (`crate::Hook`).

use crate::layouts::{Attribute, Layout};
use crate::type_ids::{AttributeType, EntityRuleType, EntityType, LayerType, ScaRuleType, WidePlane, Width, WorldgenType};

/// A type a world has: its name, and what goes with it by its kind.
#[derive(Clone, Debug)]
pub enum RegisteredType {
    /// A layer: a bit a cell.
    Layer {
        /// Its name.
        name: String,
        /// Its ID.
        layer_type: LayerType,
    },
    /// A wide plane: a number a cell, its width and tile size in its ID.
    WidePlane {
        /// Its name.
        name: String,
        /// Its ID, the plane whole.
        layer_type: LayerType,
    },
    /// A type of entity, and the attribute every entity of it has for
    /// as long as it is, if any: one of a fixed size, so that it need
    /// not say its type where it is kept.
    Entity {
        /// Its name.
        name: String,
        /// Its ID.
        entity_type: EntityType,
        /// Its default attribute.
        default_attribute: Option<AttributeType>,
    },
    /// An attribute of an entity, and the words its data is: none if
    /// its size varies, each of it then saying its own length.
    Attribute {
        /// Its name.
        name: String,
        /// Its ID.
        attribute_type: AttributeType,
        /// The words its data is.
        data_words: Option<usize>,
    },
    /// A rule of the cells, and the one plane it is of -- whatever
    /// else it reads or writes.
    ScaRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: ScaRuleType,
        /// Its master.
        master: LayerType,
    },
    /// A rule of an entity, and the one type of entity it is of.
    EntityRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: EntityRuleType,
        /// Its master.
        master: EntityType,
    },
    /// A way of generating a world: its superchunks' cells, and what
    /// lives on them.
    Worldgen {
        /// Its name.
        name: String,
        /// Its ID.
        worldgen: WorldgenType,
    },
}

impl RegisteredType {
    /// A layer named `name`.
    pub fn layer(name: &str, layer_type: LayerType) -> Self {
        Self::Layer { name: name.to_string(), layer_type }
    }

    /// A wide plane named `name`.
    pub fn wide_plane<W: Width>(name: &str, wide_plane: WidePlane<W>) -> Self {
        Self::WidePlane { name: name.to_string(), layer_type: wide_plane.layer_type() }
    }

    /// A type of entity named `name`, with `default_attribute` if any.
    pub fn entity<L: Layout>(name: &str, entity_type: EntityType, default_attribute: Option<Attribute<L>>) -> Self {
        Self::Entity { name: name.to_string(), entity_type, default_attribute: default_attribute.map(Attribute::attribute_type) }
    }

    /// An attribute named `name`, of a fixed size: its layout's.
    pub fn attribute<L: Layout>(name: &str, attribute: Attribute<L>) -> Self {
        Self::Attribute { name: name.to_string(), attribute_type: attribute.attribute_type(), data_words: Some(L::WORDS) }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        match self {
            Self::Layer { name, .. } | Self::WidePlane { name, .. } | Self::Entity { name, .. } | Self::Attribute { name, .. } | Self::ScaRule { name, .. } | Self::EntityRule { name, .. } | Self::Worldgen { name, .. } => name,
        }
    }

    /// Its layer type, if it is a layer or a wide plane.
    pub fn layer_type(&self) -> Option<LayerType> {
        match self {
            Self::Layer { layer_type, .. } | Self::WidePlane { layer_type, .. } => Some(*layer_type),
            _ => None,
        }
    }

    /// Its ID whole -- kind, number and size -- as a world records it.
    pub fn type_id(&self) -> u64 {
        match self {
            Self::Layer { layer_type, .. } | Self::WidePlane { layer_type, .. } => layer_type.0,
            Self::Entity { entity_type, .. } => entity_type.0,
            Self::Attribute { attribute_type, .. } => attribute_type.0,
            Self::ScaRule { rule, .. } => rule.0,
            Self::EntityRule { rule, .. } => rule.0,
            Self::Worldgen { worldgen, .. } => worldgen.0,
        }
    }

    /// Whether it is a rule, of the cells or of an entity.
    pub fn is_a_rule(&self) -> bool {
        matches!(self, Self::ScaRule { .. } | Self::EntityRule { .. })
    }

    /// The numbers its ID takes: a wide plane one a bit, its cold
    /// planes', anything else one.
    pub fn numbers(&self) -> std::ops::Range<u64> {
        let (first_number, numbers_taken) = match self {
            Self::Layer { layer_type, .. } => (layer_type.type_number(), 1),
            Self::WidePlane { layer_type, .. } => (layer_type.type_number(), u64::from(layer_type.width_bits())),
            Self::Entity { entity_type, .. } => (entity_type.type_number(), 1),
            Self::Attribute { attribute_type, .. } => (attribute_type.type_number(), 1),
            Self::ScaRule { rule, .. } => (rule.type_number(), 1),
            Self::EntityRule { rule, .. } => (rule.type_number(), 1),
            Self::Worldgen { worldgen, .. } => (worldgen.type_number(), 1),
        };
        first_number..first_number + numbers_taken
    }
}
