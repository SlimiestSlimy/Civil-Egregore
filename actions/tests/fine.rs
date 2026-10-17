//! The fine tier: instructions asked on a few entities -- milliseconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
#[path = "shared/arenas.rs"]
mod arenas;

mod entities {
    //! What an entity comes to, and the cells beside it: one changed is
    //! written only the attributes changed, and the nine cells about it,
    //! the free ones among them, are asked as masks.
    //!
    //! `cargo test --test fine`

    use crate::arenas::{cell, walker_header, world, STONE_LAYER};
    use coordinates::CellIndex;
    use entity_manager::{Attribute, AttributeBlock, NEVER_WAKES};
    use actions::around;
    use actions::{entities, Turn};
    use simulation::Simulation;
    use std::sync::Mutex;

    /// An attribute.
    const NAME_ATTRIBUTE: Attribute<u64> = Attribute::new(41);

    /// An entity its rule looks over and leaves as it was is moved, or put
    /// to sleep, with nothing carried; one with an attribute changed has
    /// that attribute written, held against what was seen of it, and is
    /// moved the same -- never put whole.
    #[test]
    fn an_entity_is_written_only_the_attributes_changed() {
        let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
        let (mut bitmap_arena, mut entity_store) = world(1);
        entity_store.queue_put(walker_header(cell(40, 30), 0), &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
        entity_store.queue_put(walker_header(cell(40, 40), 0), &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
        entity_store.apply();
        let mut test_simulation = Simulation::new(1);
        let report = test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn: &mut Turn| {
            for entity in turn.woken_entities() {
                // The first is left as it was; the second walker's name is changed.
                if entity.header.stood_cell == cell(40, 40) {
                    entities::set_attribute_of(turn, &entity.header, NAME_ATTRIBUTE, entity.attribute(NAME_ATTRIBUTE), 8);
                }
                let step_cell = entity.header.stood_cell.at_cartesian_offset(1, 1).expect("in the world");
                entities::step(turn, &entity.header, step_cell, NEVER_WAKES);
            }
            0
        });
        assert_eq!((report.instructions_applied.moves, report.instructions_applied.puts, report.instructions_applied.edits, report.instructions_compared), (2, 0, 1, (3, 0)));
        assert_eq!(entity_store.get(cell(41, 31)).expect("moved").attributes, [AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
        assert_eq!(entity_store.get(cell(41, 41)).expect("moved").attributes, [AttributeBlock::holding_value(NAME_ATTRIBUTE, 8)]);
    }

    /// The cells about an entity: the layer's among them, the ways to
    /// each, and one drawn among those chosen.
    #[test]
    fn the_cells_beside_an_entity_are_asked_as_sets() {
        let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
        let (mut bitmap_arena, mut entity_store) = world(1);
        let walker_cell = cell(40, 30);
        entity_store.queue_put(walker_header(walker_cell, 0), &[]);
        entity_store.apply();
        let seen_by_rule = Mutex::new(Vec::new());
        let mut test_simulation = Simulation::new(1);
        test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |turn: &mut Turn| {
            for entity in turn.woken_entities() {
                let woken_cell = entity.header.stood_cell;
                let about_tiles = around::tiles_about_cell(woken_cell);
                let stone_about = around::read_layer(turn, STONE_LAYER, &about_tiles);
                // Up and left of it, and to its right: the two chosen among.
                let two_chosen = about_tiles.cell_towards(around::NORTH_WEST) | about_tiles.cell_towards(around::EAST);
                let ways_drawn = (0..64).filter_map(|_| around::pick_way(turn.random_stream(), &about_tiles, two_chosen)).fold(0u16, |drawn_so_far, way| drawn_so_far | 1 << way);
                let none_drawn = around::pick_way(turn.random_stream(), &about_tiles, two_chosen.cells_not_in(two_chosen));
                let cells: Vec<CellIndex> = [around::NORTH_WEST, around::EAST].iter().filter_map(|&way| around::cell(woken_cell, way)).collect();
                seen_by_rule.lock().unwrap().push((stone_about.set_cells.count(), stone_about.hot == about_tiles.nine_cells(), ways_drawn, none_drawn, cells, around::way_of(woken_cell, woken_cell.at_cartesian_offset(-1, 1).unwrap())));
                entities::sleep(turn, &entity.header, NEVER_WAKES);
            }
            0
        });
        assert_eq!(*seen_by_rule.lock().unwrap(), [(0, true, 1 << 0 | 1 << 5, None, vec![cell(39, 29), cell(41, 30)], 6)]);
    }
}
