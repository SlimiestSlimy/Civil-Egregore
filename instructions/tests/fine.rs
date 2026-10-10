//! The fine tier: instructions asked on a few entities -- milliseconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod tests;

mod entities {
    //! What an entity comes to, and the cells beside it: one changed is
    //! put whole only if an attribute was, and the nine cells about it,
    //! the free ones among them, are asked as masks.
    //!
    //! `cargo test --test fine`

    use crate::tests::{cell, walker, world, STONE};
    use coordinates::CellIndex;
    use entity_manager::{Attribute, AttributeBlock, EntityEdit, EntityId, NEVER};
    use instructions::around::{self, CENTRE, RING};
    use instructions::{entities, Turn};
    use simulation::Simulation;
    use std::sync::Mutex;

    /// An attribute.
    const NAME: Attribute<u64> = Attribute::new(41);
    /// Another.
    const MARK: Attribute<u64> = Attribute::new(42);

    /// An entity its rule looks over and leaves as it was is moved, or put
    /// to sleep, with nothing carried; one with an attribute changed is put
    /// whole.
    #[test]
    fn an_entity_is_put_whole_only_if_an_attribute_changed() {
        let (mut arena, mut entities) = world(1);
        entities.queue_put(walker(1, cell(40, 30), 0), &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(2, cell(40, 40), 0), &[AttributeBlock::holding(NAME, 7)]);
        entities.apply();
        let mut simulation = Simulation::new(1);
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            let mut room = Vec::new();
            for entity in turn.woken() {
                let mut edit = EntityEdit::of(entity, &mut room);
                // Set to what it is: no change. The second walker's is changed, and another added and removed.
                edit.set(NAME, 7);
                assert_eq!(edit.unset(MARK), None);
                assert!(!edit.edited());
                if entity.header.id == EntityId(2) {
                    edit.set(NAME, 8);
                    edit.set(MARK, 1);
                    assert_eq!((edit.get(NAME), edit.get(MARK), edit.unset(MARK)), (Some(8), Some(1), Some(1)));
                }
                let to = entity.header.at.offset(1, 1).expect("in the world");
                entities::commit(turn, edit, to, NEVER);
            }
            0
        });
        assert_eq!((report.instructions_applied.moves, report.instructions_applied.puts), (1, 1));
        assert_eq!(entities.get(EntityId(1), cell(41, 31)).expect("moved").attributes, [AttributeBlock::holding(NAME, 7)]);
        assert_eq!(entities.get(EntityId(2), cell(41, 41)).expect("moved").attributes, [AttributeBlock::holding(NAME, 8)]);
    }

    /// The nine cells about an entity as bits: which entities stand on,
    /// which are free among those open to it, and none when all are taken.
    #[test]
    fn the_cells_beside_an_entity_are_asked_as_masks() {
        let (mut arena, mut entities) = world(1);
        let at = cell(40, 30);
        entities.queue_put(walker(1, at, 0), &[]);
        // Up and left of it, and to its right.
        entities.queue_put(walker(2, cell(39, 29), NEVER), &[]);
        entities.queue_put(walker(3, cell(41, 30), NEVER), &[]);
        entities.apply();
        let seen = Mutex::new(Vec::new());
        let mut simulation = Simulation::new(1);
        simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                let at = entity.header.at;
                let (stone, taken) = (around::layer(turn, STONE, at), around::occupied(turn, at));
                let free = (0..64).filter_map(|_| around::free_beside(turn, at, RING)).fold(0u16, |free, bit| free | 1 << bit);
                let only = around::free_beside(turn, at, 1 << 0 | 1 << 5);
                let cells: Vec<CellIndex> = [0, 5].iter().filter_map(|&bit| around::cell(at, bit)).collect();
                seen.lock().unwrap().push((stone.set, stone.hot, taken, free, only, cells, around::bit_of(at, at.offset(-1, 1).unwrap())));
                entities::sleep(turn, &entity.header, NEVER);
            }
            0
        });
        let taken = 1 << 0 | CENTRE | 1 << 5;
        assert_eq!(*seen.lock().unwrap(), [(0, around::ALL, taken, RING & !taken, None, vec![cell(39, 29), cell(41, 30)], 6)]);
    }
}
