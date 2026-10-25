//! The world's render rules, gathered once its types are frozen
//! ([`RenderRules`]), and a superchunk painted with them: a picture a
//! render layer, each rule of the layer painting over those before it
//! (`client_api::rendering`).

use crate::client_api::rendering::{drawn_side, RenderCode, RenderContext, RenderTurn};
use protocol::pipe::frame::Cells;
use protocol::type_registry::{Drawn, FrozenTypeRegistry, PlaneType, RegisteredType};

/// A world's render rules, by render layer, and the planes they read.
pub struct RenderRules {
    /// Each render layer, lowest first, and its rules, in the order
    /// they were registered.
    draw_orders: Vec<(u32, Vec<(RenderCode, RenderContext)>)>,
    /// The planes the rules draw or read, each once: what a frame is asked for.
    pub planes: Vec<PlaneType>,
}

/// A superchunk's picture of one render layer: RGBA, a drawn cell a
/// pixel, row by row -- none if nothing was painted in it.
pub type Picture = Option<Vec<u8>>;

impl RenderRules {
    /// The render rules of `frozen_type_registry`: those whose code it
    /// holds -- a game built headless registers them without.
    pub fn of(frozen_type_registry: &FrozenTypeRegistry) -> Self {
        let mut draw_orders: Vec<(u32, Vec<(RenderCode, RenderContext)>)> = Vec::new();
        let mut planes = Vec::new();
        for registered_type in frozen_type_registry.types() {
            let RegisteredType::RenderRule { master, draw_order, .. } = registered_type else {
                continue;
            };
            let Some(render_code) = frozen_type_registry.library_of(registered_type).and_then(|library| library.downcast_ref::<RenderCode>()) else {
                continue;
            };
            if let Drawn::Plane(plane_type) = *master {
                planes.push(plane_type);
            }
            planes.extend(frozen_type_registry.dependencies_of(registered_type).filter_map(RegisteredType::plane_type));
            let rule = (render_code.clone(), RenderContext { master: *master });
            match draw_orders.binary_search_by_key(draw_order, |(layer, _)| *layer) {
                Ok(found) => draw_orders[found].1.push(rule),
                Err(place) => draw_orders.insert(place, (*draw_order, vec![rule])),
            }
        }
        planes.sort_unstable_by_key(|plane_type| plane_type.type_id());
        planes.dedup();
        Self { draw_orders, planes }
    }

    /// Each render layer, lowest first.
    pub fn draw_orders(&self) -> impl Iterator<Item = u32> + '_ {
        self.draw_orders.iter().map(|(draw_order, _)| *draw_order)
    }

    /// `cells` painted at `detail_level`: each render layer's picture, in
    /// the order of [`RenderRules::draw_orders`].
    pub fn paint(&self, cells: &Cells, detail_level: u32) -> Vec<Picture> {
        let picture_bytes = 4 * (drawn_side(detail_level) as usize).pow(2);
        self.draw_orders
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
