//! The fine tier: every message a client and a server send each other,
//! drawn from the seed, comes off the wire as it went on -- the same
//! words put again.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;

use protocol::pipe::frame::{Cells, FrameAsk, NearRectangle, Viewport, ViewportFrame};
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use protocol::pipe::terrain::{CellCover, HeightsAsk, MapAnswer, MapAsk, MapCell};
use protocol::type_registry::LayerType;
use utilities::rng::RandomSource;
use utilities::wire::{receive, send, Wired};

/// `count` words drawn from `random`.
fn drawn_words(random: &mut RandomSource, count: u64) -> Vec<u64> {
    (0..count).map(|_| random.next_draw()).collect()
}

/// Up to `most` layer types drawn from `random`, of every shape.
fn drawn_layer_types(random: &mut RandomSource, most: u64) -> Vec<LayerType> {
    (0..random.draw_below(most + 1)).map(|_| LayerType::of_tiles(random.draw_below(protocol::type_registry::TYPE_ID_NUMBERS - 32), 1 << random.draw_below(6), random.draw_below(4) as u32)).collect()
}

/// A pair drawn from `random`.
fn drawn_pair(random: &mut RandomSource) -> (u32, u32) {
    (random.next_draw() as u32, random.next_draw() as u32)
}

/// A frame ask drawn from `random`.
fn drawn_ask(random: &mut RandomSource) -> FrameAsk {
    let viewport = random.percent_chance(50).then(|| Viewport { top_left_superchunk_cartesian: drawn_pair(random), bottom_right_superchunk_cartesian: drawn_pair(random) });
    let seen_near = random.percent_chance(50).then(|| NearRectangle { first_cell_cartesian: drawn_pair(random), size_in_cells: drawn_pair(random), pixels_a_cell: random.draw_below(9) as u32 });
    FrameAsk { viewport, pixel_side_log2: random.draw_below(8) as u32, superchunks_skipped: random.next_draw() as u32, most_superchunks: random.next_draw() as u32, seen_near, planes: drawn_layer_types(random, 8), counted: drawn_layer_types(random, 3) }
}

/// A superchunk's cells drawn from `random`, some planes empty.
fn drawn_cells(random: &mut RandomSource) -> Cells {
    let plane = |random: &mut RandomSource| {
        let count = random.draw_below(3) * random.draw_below(64);
        drawn_words(random, count)
    };
    let planes = drawn_layer_types(random, 8).into_iter().map(|layer_type| (layer_type, plane(random))).collect();
    Cells { superchunk_cartesian: drawn_pair(random), planes, top_left: drawn_pair(random), entity_cells_cartesian: (0..random.draw_below(30)).map(|_| (random.next_draw() as u16, random.next_draw() as u16)).collect() }
}

/// A frame drawn from `random`.
fn drawn_frame(random: &mut RandomSource) -> ViewportFrame {
    let ask = drawn_ask(random);
    ViewportFrame {
        world: random.next_draw(),
        seed: random.next_draw(),
        tick: random.next_draw(),
        ticks_a_second: f64::from_bits(random.next_draw()),
        target_pace: random.next_draw() as u32,
        held_pace: random.percent_chance(50).then(|| random.next_draw() as u32),
        entity_count: random.draw_below(1 << 40) as usize,
        counts: drawn_layer_types(random, 3).into_iter().map(|layer_type| (layer_type, random.next_draw())).collect(),
        side_superchunks: random.percent_chance(50).then(|| random.next_draw() as u32),
        hot: (0..random.draw_below(20)).map(|_| drawn_pair(random)).collect(),
        viewport: ask.viewport,
        seen_near: ask.seen_near,
        world_name: random.percent_chance(50).then(|| format!("world {}", random.next_draw())),
        last_outcome: random.percent_chance(50).then(|| "opened".to_string()),
        more_to_come: random.percent_chance(50),
        cells: (0..random.draw_below(6)).map(|_| drawn_cells(random)).collect(),
        ..ViewportFrame::default()
    }
}

/// Every kind of message each way, drawn from `random`.
fn drawn_messages(random: &mut RandomSource) -> (Vec<ClientMessage>, Vec<ServerMessage>) {
    let tuning = std::array::from_fn(|_| f32::from_bits(random.next_draw() as u32));
    let heights_ask = HeightsAsk { world: random.next_draw(), first_cell_cartesian: drawn_pair(random), size_in_cells: drawn_pair(random), skipped_corners_cartesian: (0..random.draw_below(4)).map(|_| drawn_pair(random)).collect() };
    let map_ask = MapAsk { world: random.next_draw(), first_cell_cartesian: (random.next_draw() as i64, random.next_draw() as i64), step: random.next_draw() as u32, size_in_pixels: drawn_pair(random), mesh_lines_asked: random.percent_chance(50) };
    let covers = [CellCover::Ocean, CellCover::Tree, CellCover::Grass, CellCover::Dirt];
    let map_cells = (0..random.draw_below(50)).map(|_| random.percent_chance(80).then(|| MapCell { height: random.next_draw() as u16, height_above: random.next_draw() as u16, cell_cover: covers[random.draw_below(4) as usize], on_a_mesh_line: random.percent_chance(50) })).collect();
    let client = vec![
        ClientMessage::AskFrame(drawn_ask(random)),
        ClientMessage::SetPaused(random.percent_chance(50)),
        ClientMessage::SetPace(random.percent_chance(50).then(|| random.next_draw() as u32)),
        ClientMessage::MakeWorld(random.percent_chance(50).then(|| random.next_draw()), tuning),
        ClientMessage::OpenWorld("a world's name, ünïcode and all".to_string()),
        ClientMessage::Save(String::new()),
        ClientMessage::Reset(tuning),
        ClientMessage::AskHeights(heights_ask),
        ClientMessage::AskMap(map_ask),
        ClientMessage::ListWorlds,
    ];
    let server = vec![
        ServerMessage::Frame(Box::new(drawn_frame(random))),
        ServerMessage::Heights(random.percent_chance(50).then(|| (0..random.draw_below(100)).map(|_| random.next_draw() as u16).collect())),
        ServerMessage::Map(random.percent_chance(80).then(|| MapAnswer { map_ask, drawn_heights: Default::default(), cells: map_cells })),
        ServerMessage::Worlds(vec!["one".to_string(), "two".to_string()]),
    ];
    (client, server)
}

/// The words `message` is put as.
fn words_of(message: &mut impl Wired) -> Vec<u64> {
    let mut words = Vec::new();
    message.put(&mut words);
    words
}

/// Each message sent down a stream comes back off it put as the same
/// words, in order; and nothing comes after the last.
#[test]
fn every_message_comes_off_the_wire_as_it_went_on() {
    let mut random = RandomSource::new(test_seed::TestSeedHandle::open().fine_seed());
    for _ in 0..20 {
        let (mut client, mut server) = drawn_messages(&mut random);
        let mut stream = Vec::new();
        client.iter_mut().for_each(|message| send(message, &mut stream).expect("sent to memory"));
        let mut reader = stream.as_slice();
        for message in &mut client {
            let mut received: ClientMessage = receive(&mut reader).expect("read").expect("a message");
            assert_eq!(words_of(&mut received), words_of(message));
        }
        assert!(receive::<ClientMessage>(&mut reader).expect("read").is_none());
        stream.clear();
        server.iter_mut().for_each(|message| send(message, &mut stream).expect("sent to memory"));
        let mut reader = stream.as_slice();
        for message in &mut server {
            let mut received: ServerMessage = receive(&mut reader).expect("read").expect("a message");
            assert_eq!(words_of(&mut received), words_of(message));
        }
    }
}
