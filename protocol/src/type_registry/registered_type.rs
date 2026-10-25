//! A type a world has: a name, and what goes with it by its kind.

use super::registration::BundleSide;
use super::type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, PlaneType, RenderRuleType, ScaRuleType, WorldgenType};

/// A type a world has: its name, and what goes with it by its kind.
#[derive(Clone, Debug)]
pub enum RegisteredType {
    /// A plane: a number a cell, or a tile, its width and tile size in
    /// its plane type, as registered.
    Plane {
        /// Its name.
        name: String,
        /// Its ID and shape.
        plane_type: PlaneType,
    },
    /// A type of entity, the attribute every entity of it has for as long
    /// as it is, if any -- one of a fixed size, so that it need not say
    /// its type where it is kept -- and the rule run as one wakes, if any.
    Entity {
        /// Its name.
        name: String,
        /// Its ID.
        entity_type: EntityType,
        /// Its default attribute.
        default_attribute: Option<AttributeType>,
        /// The entity rule run as one of it wakes.
        rule: Option<EntityRuleType>,
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
    /// A rule of the cells.
    ScaRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: ScaRuleType,
    },
    /// A rule of entities.
    EntityRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: EntityRuleType,
    },
    /// A way of generating a world: its superchunks' cells, and what
    /// lives on them.
    Worldgen {
        /// Its name.
        name: String,
        /// Its ID.
        worldgen: WorldgenType,
    },
    /// A rule of the renderer.
    RenderRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: RenderRuleType,
        /// Where it is drawn among the render rules: the higher over the lower.
        draw_order: u32,
    },
    /// A window of the client's, and the window it is nested in, if any.
    Gui {
        /// Its name.
        name: String,
        /// Its ID.
        gui: GuiType,
        /// Its parent.
        parent: Option<GuiType>,
    },
    /// A function a client may call across the connection.
    ClientCall {
        /// Its name.
        name: String,
        /// Its ID: what a call names on the wire.
        call: ClientCallType,
    },
    /// A bundle: the types whose names begin with its own are in it.
    Bundle {
        /// Its name, whole: its outer bundle's, if any, then its own.
        name: String,
        /// Its ID.
        bundle: BundleType,
        /// The side its code runs on: none for a mod's own bundle.
        side: Option<BundleSide>,
        /// The bundles it depends on.
        depends_on: Box<[BundleType]>,
    },
}

impl RegisteredType {
    /// Its name.
    pub fn name(&self) -> &str {
        match self {
            Self::Plane { name, .. } | Self::Entity { name, .. } | Self::Attribute { name, .. } | Self::ScaRule { name, .. } | Self::EntityRule { name, .. } | Self::Worldgen { name, .. } | Self::RenderRule { name, .. } | Self::Gui { name, .. } | Self::ClientCall { name, .. } | Self::Bundle { name, .. } => name,
        }
    }

    /// Its plane type, if it is a plane.
    pub fn plane_type(&self) -> Option<PlaneType> {
        match self {
            Self::Plane { plane_type, .. } => Some(*plane_type),
            _ => None,
        }
    }

    /// Its type of entity, if it is one.
    pub fn entity_type(&self) -> Option<EntityType> {
        match self {
            Self::Entity { entity_type, .. } => Some(*entity_type),
            _ => None,
        }
    }

    /// Its attribute's type, if it is an attribute.
    pub fn attribute_type(&self) -> Option<AttributeType> {
        match self {
            Self::Attribute { attribute_type, .. } => Some(*attribute_type),
            _ => None,
        }
    }

    /// Its ID whole -- kind and number -- as a world records it.
    pub fn type_id(&self) -> u64 {
        match self {
            Self::Plane { plane_type, .. } => plane_type.type_id(),
            Self::Entity { entity_type, .. } => entity_type.0,
            Self::Attribute { attribute_type, .. } => attribute_type.type_id(),
            Self::ScaRule { rule, .. } => rule.0,
            Self::EntityRule { rule, .. } => rule.0,
            Self::Worldgen { worldgen, .. } => worldgen.0,
            Self::RenderRule { rule, .. } => rule.0,
            Self::Gui { gui, .. } => gui.0,
            Self::ClientCall { call, .. } => call.0,
            Self::Bundle { bundle, .. } => bundle.0,
        }
    }

    /// Whether it is a rule the simulation ticks, of the cells or of an
    /// entity: not one of the renderer's.
    pub fn is_a_rule(&self) -> bool {
        matches!(self, Self::ScaRule { .. } | Self::EntityRule { .. })
    }

    /// Its number: what its ID holds under its kind, no other type's.
    pub fn type_number(&self) -> u64 {
        self.type_id() & (super::TYPE_ID_NUMBERS - 1)
    }
}
