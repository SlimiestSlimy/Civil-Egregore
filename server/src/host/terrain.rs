//! The terrain a client asks of the host where no frame brings it
//! (`protocol::pipe::terrain`), answered on a thread of its own, the
//! ticks never waiting, by the world run's generation
//! ([`WorldgenCode`]) (`docs/server.md`, "Terrain asked of the host").

use protocol::pipe::terrain::{AsksTerrain, Height, HeightsAsk, MapAnswer, MapAsk};
use crate::simulation::simulation_api::WorldgenCode;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// What the terrain's thread is sent.
pub(crate) enum TerrainRequest {
    /// The world run from now, by the host's own thread.
    World(WorldTold),
    /// Heights asked for, and where to answer.
    Heights(HeightsAsk, Sender<Option<Vec<Height>>>),
    /// A map asked for, and where to answer.
    Map(MapAsk, Sender<Option<MapAnswer>>),
}

/// The world the terrain's thread answers of: what the host last told it.
pub(crate) struct WorldTold {
    /// The world's number, as the host counts the worlds it runs.
    pub world: u64,
    /// Its seed.
    pub seed: u64,
    /// Its generation's numbers.
    pub generation_numbers: Vec<(String, u64)>,
    /// What generates it.
    pub worldgen: WorldgenCode,
}

/// Where a client asks of the terrain of the world the host runs: each
/// call waits for its answer, on the caller's thread -- a thread other
/// than the window's own asks what takes long.
#[derive(Clone)]
pub struct TerrainAsker {
    /// Where the asks go.
    pub(crate) request_sender: Sender<TerrainRequest>,
}

impl AsksTerrain for TerrainAsker {
    fn heights(&self, heights_ask: HeightsAsk) -> Option<Vec<Height>> {
        let (answer_sender, answer_receiver) = channel();
        self.request_sender.send(TerrainRequest::Heights(heights_ask, answer_sender)).ok()?;
        answer_receiver.recv().ok().flatten()
    }

    fn map(&self, map_ask: MapAsk) -> Option<MapAnswer> {
        let (answer_sender, answer_receiver) = channel();
        self.request_sender.send(TerrainRequest::Map(map_ask, answer_sender)).ok()?;
        answer_receiver.recv().ok().flatten()
    }
}

/// Starts the terrain's thread: where it is asked. It stops once every
/// asker and the host are gone.
pub(crate) fn start_terrain_thread() -> Sender<TerrainRequest> {
    let (request_sender, request_receiver) = channel();
    thread::Builder::new().name("terrain".to_string()).spawn(move || answer_terrain_requests(&request_receiver)).expect("a thread for the terrain");
    request_sender
}

/// Answers what is asked of the world last told of, until no one is
/// left to ask.
fn answer_terrain_requests(request_receiver: &Receiver<TerrainRequest>) {
    let mut world_told: Option<WorldTold> = None;
    while let Ok(terrain_request) = request_receiver.recv() {
        let running = |world: u64| world_told.as_ref().filter(|told| told.world == world);
        // An asker gone before its answer is nothing to tell anyone.
        match terrain_request {
            TerrainRequest::World(told) => world_told = Some(told),
            TerrainRequest::Heights(heights_ask, heights_answer) => _ = heights_answer.send(running(heights_ask.world).map(|told| (told.worldgen.heights)(&told.generation_numbers, told.seed, &heights_ask))),
            TerrainRequest::Map(map_ask, map_answer) => _ = map_answer.send(running(map_ask.world).map(|told| (told.worldgen.map)(&told.generation_numbers, told.seed, map_ask))),
        }
    }
}
