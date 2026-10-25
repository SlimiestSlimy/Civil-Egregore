//! The render rules' loops (`client_api::rendering`): which drawn cells
//! each visits at each detail level, what it reads of them, and what it
//! paints.

use crate::client_api::rendering::{drawn_side, on_each_entity, on_each_set_cell, DrawnCell, RenderTurn, SUPERCHUNK_SIDE_LOG2};
use bevy::color::Color;
use protocol::pipe::frame::{Cells, CHUNK_WORDS, WORD_BITS};
use protocol::type_registry::{EntityType, PlaneTileSize, PlaneType, PlaneWidth, Registration, TypeRegistry};
use utilities::coordinates::{CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};

/// The tests' bundle.
const TESTS_BUNDLE: &str = "tests";

/// A layer and a type of entity, as a new world's registry gives them their IDs.
fn registered_types() -> (PlaneType, EntityType) {
    let mut type_registry = TypeRegistry::for_new_world();
    let mut tests = type_registry.open_bundle(TESTS_BUNDLE).expect("the tests' bundle");
    let layer = tests.register(Registration::plane("grass", PlaneWidth::Bits1, PlaneTileSize::Side1)).expect("a layer").plane_type().expect("a layer");
    let entity = tests.register(Registration::entity("sheep", None)).expect("a type of entity").entity_type().expect("a type of entity");
    (layer, entity)
}

/// A superchunk's cells with `layer` set at each of `set_cells`, `(x, y)`
/// from its top left, and an entity of `entity_type` at each of `entity_cells`.
fn cells_with(layer: PlaneType, set_cells: &[(u32, u32)], entity_type: EntityType, entity_cells: &[(u16, u16)]) -> Cells {
    let mut words = vec![0u64; CHUNKS_IN_SUPERCHUNK * CHUNK_WORDS];
    for &(x, y) in set_cells {
        let bit = DrawnCell::at(x, y).morton_index;
        words[bit / WORD_BITS] |= 1 << (bit % WORD_BITS);
    }
    Cells { planes: vec![(layer, words)], entities: entity_cells.iter().map(|&cell| (entity_type, cell)).collect(), ..Cells::default() }
}

/// What `look` saw of `cells` on a turn at `detail_level`, and the picture painted.
fn on_a_turn<T>(cells: &Cells, detail_level: u32, look: impl FnOnce(&mut RenderTurn) -> T) -> (T, Vec<u8>) {
    let mut picture = vec![0; 4 * (drawn_side(detail_level) as usize).pow(2)];
    let seen = look(&mut RenderTurn::new(cells, detail_level, &mut picture));
    (seen, picture)
}

/// Each drawn cell visited, `(x, y)`, and how many of its cells are set.
type Visited = Vec<((u32, u32), u32)>;

/// The cells set in each test: two in one 2-by-2 square, one further
/// along the top row, and the far corner.
const SET_CELLS: [(u32, u32); 4] = [(0, 0), (1, 1), (5, 0), (SUPERCHUNK_SIDE_CELLS - 1, SUPERCHUNK_SIDE_CELLS - 1)];

#[test]
fn a_drawn_cell_is_where_it_says() {
    for (x, y) in [(0, 0), (1, 0), (0, 1), (5, 3), (SUPERCHUNK_SIDE_CELLS - 1, 0), (SUPERCHUNK_SIDE_CELLS - 1, SUPERCHUNK_SIDE_CELLS - 1)] {
        assert_eq!(DrawnCell::at(x, y).cartesian(), (x, y), "({x}, {y}) and back");
    }
    assert_eq!(DrawnCell::at(1, 0).morton_index, 1, "x in the even bits");
    assert_eq!(DrawnCell::at(0, 1).morton_index, 2, "y in the odd bits");
}

#[test]
fn each_drawn_cell_with_a_set_cell_is_visited_once_with_its_share() {
    let (layer, entity_type) = registered_types();
    let cells = cells_with(layer, &SET_CELLS, entity_type, &[]);
    // At each level: each drawn cell visited, `(x, y)`, and how many of its cells are set.
    let far_corner = SUPERCHUNK_SIDE_CELLS - 1;
    let cases: [(u32, Visited); 4] = [
        (0, vec![((0, 0), 1), ((1, 1), 1), ((5, 0), 1), ((far_corner, far_corner), 1)]),
        (1, vec![((0, 0), 2), ((2, 0), 1), ((far_corner >> 1, far_corner >> 1), 1)]),
        (3, vec![((0, 0), 3), ((far_corner >> 3, far_corner >> 3), 1)]),
        (SUPERCHUNK_SIDE_LOG2, vec![((0, 0), 4)]),
    ];
    for (detail_level, expected) in cases {
        let (visited, _) = on_a_turn(&cells, detail_level, |turn| {
            let mut visited = Vec::new();
            on_each_set_cell(turn, layer, |turn, drawn| {
                visited.push((drawn, turn.count_set(layer, drawn)));
                None
            });
            visited
        });
        let mut visited: Visited = visited.into_iter().map(|(drawn, (set, of))| {
            assert_eq!(of, 1 << (2 * detail_level), "a drawn cell's cells at level {detail_level}");
            (drawn.cartesian(), set)
        }).collect();
        visited.sort_unstable();
        let mut expected = expected;
        expected.sort_unstable();
        assert_eq!(visited, expected, "at level {detail_level}");
    }
}

#[test]
fn a_set_cell_is_painted_where_it_stands() {
    let (layer, entity_type) = registered_types();
    let cells = cells_with(layer, &[(5, 3)], entity_type, &[]);
    let (_, picture) = on_a_turn(&cells, 0, |turn| on_each_set_cell(turn, layer, |_, _| Some(Color::WHITE)));
    let pixel = 4 * (3 * SUPERCHUNK_SIDE_CELLS + 5) as usize;
    assert_eq!(&picture[pixel..pixel + 4], &[255; 4], "the cell's pixel, row by row");
    assert_eq!(picture.iter().filter(|&&byte| byte != 0).count(), 4, "and nothing else");
}

#[test]
fn entities_cover_a_drawn_cell_by_the_share_of_its_cells_they_stand_on() {
    let (layer, entity_type) = registered_types();
    let cells = cells_with(layer, &[], entity_type, &[(0, 0), (1, 0), (6, 6)]);
    let (_, picture) = on_a_turn(&cells, 1, |turn| on_each_entity(turn, entity_type, |_, _| Some(Color::WHITE)));
    let alpha_at = |(x, y): (u32, u32)| picture[4 * (y * drawn_side(1) + x) as usize + 3];
    assert_eq!(alpha_at((0, 0)), 128, "two of four cells");
    assert_eq!(alpha_at((3, 3)), 64, "one of four");
    assert_eq!(alpha_at((1, 0)), 0, "none");
    let (_, picture) = on_a_turn(&cells, 0, |turn| on_each_entity(turn, entity_type, |_, _| Some(Color::WHITE)));
    assert_eq!(picture[4 * (6 * SUPERCHUNK_SIDE_CELLS + 6) as usize + 3], 255, "seen near, an entity's cell its own");
}
