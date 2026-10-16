//! The fine tier: instructions asked on a few entities -- milliseconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

mod entities {
    //! What an entity comes to, and the cells beside it: one changed is
    //! written only the attributes changed, and the nine cells about it,
    //! the free ones among them, are asked as masks.
    //!
    //! `cargo test --test fine`

    use crate::tests::{cell, walker, world, STONE};
    use coordinates::CellIndex;
    use entity_manager::{Attribute, AttributeBlock, NEVER};
    use actions::around;
    use actions::{entities, Turn};
    use simulation::Simulation;
    use std::sync::Mutex;

    /// An attribute.
    const NAME: Attribute<u64> = Attribute::new(41);

    /// An entity its rule looks over and leaves as it was is moved, or put
    /// to sleep, with nothing carried; one with an attribute changed has
    /// that attribute written, held against what was seen of it, and is
    /// moved the same -- never put whole.
    #[test]
    fn an_entity_is_written_only_the_attributes_changed() {
        let (mut arena, mut entities) = world(1);
        entities.queue_put(walker(cell(40, 30), 0), &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(cell(40, 40), 0), &[AttributeBlock::holding(NAME, 7)]);
        entities.apply();
        let mut simulation = Simulation::new(1);
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
            for entity in turn.woken() {
                // The first is left as it was; the second walker's name is changed.
                if entity.header.at == cell(40, 40) {
                    entities::set_attribute_of(turn, &entity.header, NAME, entity.attribute(NAME), 8);
                }
                let to = entity.header.at.offset(1, 1).expect("in the world");
                entities::step(turn, &entity.header, to, NEVER);
            }
            0
        });
        assert_eq!((report.instructions_applied.moves, report.instructions_applied.puts, report.instructions_applied.edits, report.instructions_compared), (2, 0, 1, (3, 0)));
        assert_eq!(entities.get(cell(41, 31)).expect("moved").attributes, [AttributeBlock::holding(NAME, 7)]);
        assert_eq!(entities.get(cell(41, 41)).expect("moved").attributes, [AttributeBlock::holding(NAME, 8)]);
    }

    /// The cells about an entity: the layer's among them, the ways to
    /// each, and one drawn among those chosen.
    #[test]
    fn the_cells_beside_an_entity_are_asked_as_sets() {
        let (mut arena, mut entities) = world(1);
        let at = cell(40, 30);
        entities.queue_put(walker(at, 0), &[]);
        entities.apply();
        let seen = Mutex::new(Vec::new());
        let mut simulation = Simulation::new(1);
        simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
            for entity in turn.woken() {
                let at = entity.header.at;
                let about = around::about(at);
                let stone = around::read_layer(turn, STONE, &about);
                // Up and left of it, and to its right: the two chosen among.
                let two = about.towards(around::NORTH_WEST) | about.towards(around::EAST);
                let drawn = (0..64).filter_map(|_| around::pick(turn.random(), &about, two)).fold(0u16, |drawn, way| drawn | 1 << way);
                let none = around::pick(turn.random(), &about, two.without(two));
                let cells: Vec<CellIndex> = [around::NORTH_WEST, around::EAST].iter().filter_map(|&way| around::cell(at, way)).collect();
                seen.lock().unwrap().push((stone.set.count(), stone.hot == about.nine(), drawn, none, cells, around::way_of(at, at.offset(-1, 1).unwrap())));
                entities::sleep(turn, &entity.header, NEVER);
            }
            0
        });
        assert_eq!(*seen.lock().unwrap(), [(0, true, 1 << 0 | 1 << 5, None, vec![cell(39, 29), cell(41, 30)], 6)]);
    }
}
