//! A type a world has: a name, and what goes with it by its kind. What
//! runs it, if anything does, is its library, hooked in by its
//! registration (`super::Hook`).

use super::type_ids::{AttributeType, BundleType, ClientCallType, EntityRuleType, EntityType, GuiType, LayerType, RenderRuleType, ScaRuleType, WorldgenType};

/// What a rule of the renderer draws: a plane, or a type of entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawn {
    /// A layer or a wide plane.
    Plane(LayerType),
    /// A type of entity.
    Entity(EntityType),
}

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
    /// A wide plane: a number a cell, its width and tile size in its
    /// layer type, as registered.
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
    /// A rule of the renderer, and the one plane or type of entity it
    /// draws.
    RenderRule {
        /// Its name.
        name: String,
        /// Its ID.
        rule: RenderRuleType,
        /// Its master.
        master: Drawn,
        /// Where it is drawn among the render layers.
        render_layer: u32,
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
    /// A function a client may call across the connection, and the type
    /// it belongs to.
    ClientCall {
        /// Its name.
        name: String,
        /// Its ID: what a call names on the wire.
        call: ClientCallType,
        /// Its master's ID, whole.
        master: u64,
    },
    /// A bundle: the types whose names begin with its own are in it.
    Bundle {
        /// Its name, whole: its outer bundle's, if any, then its own.
        name: String,
        /// Its ID.
        bundle: BundleType,
    },
}

impl RegisteredType {
    /// Its name.
    pub fn name(&self) -> &str {
        match self {
            Self::Layer { name, .. } | Self::WidePlane { name, .. } | Self::Entity { name, .. } | Self::Attribute { name, .. } | Self::ScaRule { name, .. } | Self::EntityRule { name, .. } | Self::Worldgen { name, .. } | Self::RenderRule { name, .. } | Self::Gui { name, .. } | Self::ClientCall { name, .. } | Self::Bundle { name, .. } => name,
        }
    }

    /// Its layer type, if it is a layer or a wide plane.
    pub fn layer_type(&self) -> Option<LayerType> {
        match self {
            Self::Layer { layer_type, .. } | Self::WidePlane { layer_type, .. } => Some(*layer_type),
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
            Self::Layer { layer_type, .. } | Self::WidePlane { layer_type, .. } => layer_type.type_id(),
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
