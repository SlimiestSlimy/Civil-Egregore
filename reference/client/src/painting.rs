//! The world's render rules, gathered once its types are frozen
//! ([`RenderRules`]), and a superchunk painted with them: a picture a
//! render layer, each rule of the layer painting over those before it
//! (`client_api::rendering`).

use crate::client_api::rendering::{drawn_side, RenderCode, RenderContext, RenderTurn};
use protocol::pipe::frame::Cells;
use protocol::type_registry::{Drawn, FrozenTypeRegistry, LayerType, RegisteredType};

/// A world's render rules, by render layer, and the planes they read.
pub struct RenderRules {
    /// Each render layer, lowest first, and its rules, in the order
    /// they were registered.
    render_layers: Vec<(u32, Vec<(RenderCode, RenderContext)>)>,
    /// The planes the rules draw or read, each once: what a frame is asked for.
    pub planes: Vec<LayerType>,
}

/// A superchunk's picture of one render layer: RGBA, a drawn cell a
/// pixel, row by row -- none if nothing was painted in it.
pub type Picture = Option<Vec<u8>>;

impl RenderRules {
    /// The render rules of `frozen_type_registry`: those whose code it
    /// holds -- a game built headless registers them without.
    pub fn of(frozen_type_registry: &FrozenTypeRegistry) -> Self {
        let mut render_layers: Vec<(u32, Vec<(RenderCode, RenderContext)>)> = Vec::new();
        let mut planes = Vec::new();
        for registered_type in frozen_type_registry.types() {
            let RegisteredType::RenderRule { master, render_layer, .. } = registered_type else {
                continue;
            };
            let Some(render_code) = frozen_type_registry.library_of(registered_type).and_then(|library| library.downcast_ref::<RenderCode>()) else {
                continue;
            };
            if let Drawn::Plane(layer_type) = *master {
                planes.push(layer_type);
            }
            planes.extend(frozen_type_registry.dependencies_of(registered_type).filter_map(RegisteredType::layer_type));
            let rule = (render_code.clone(), RenderContext { master: *master });
            match render_layers.binary_search_by_key(render_layer, |(layer, _)| *layer) {
                Ok(found) => render_layers[found].1.push(rule),
                Err(place) => render_layers.insert(place, (*render_layer, vec![rule])),
            }
        }
        planes.sort_unstable_by_key(|layer_type| layer_type.type_id());
        planes.dedup();
        Self { render_layers, planes }
    }

    /// Each render layer, lowest first.
    pub fn render_layers(&self) -> impl Iterator<Item = u32> + '_ {
        self.render_layers.iter().map(|(render_layer, _)| *render_layer)
    }

    /// `cells` painted at `detail_level`: each render layer's picture, in
    /// the order of [`RenderRules::render_layers`].
    pub fn paint(&self, cells: &Cells, detail_level: u32) -> Vec<Picture> {
        let picture_bytes = 4 * (drawn_side(detail_level) as usize).pow(2);
        self.render_layers
            .iter()
            .map(|(_, rules)| {
                let mut picture = vec![0; picture_bytes];
                let mut painted = false;
                for (render_code, render_context) in rules {
                    let mut turn = RenderTurn::new(cells, detail_level, &mut picture);
                    (render_code.run)(&mut turn, render_context);
                    painted |= turn.painted;
                }
                painted.then_some(picture)
            })
            .collect()
    }
}
