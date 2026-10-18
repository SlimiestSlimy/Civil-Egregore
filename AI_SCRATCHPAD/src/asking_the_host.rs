//! What the probes share: a host -- of this process, or a server over
//! the pipe as a window has one -- asked for frames until one answers
//! the question.

use basegame::base_types::BaseTypes;
use protocol::type_registry::{FrozenTypes, TypeRegistry};
use std::sync::{Arc, OnceLock};
use protocol::pipe::frame::{FrameAsk, ViewportFrame, Viewport};
use protocol::type_registry::LayerType;
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use server::host::Host;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use protocol::server_parameters::{CleanServerParameters, ParameterValue, CAMERA_LOADS, SEED, SIDE, WORLDGEN};
use utilities::tuning::Tuning;
use utilities::wire::{receive, send};
use crate::ProbeParameters;

/// The planes a window asks for of each superchunk drawn coarser than
/// a cell a pixel: what a probe asks, so a frame costs what a window's does.
pub fn drawn_planes() -> Vec<LayerType> {
    let base_types = base_types().1;
    vec![base_types.grass, base_types.tree, base_types.wet]
}

/// The layers a window counts over the world.
pub fn counted() -> [LayerType; 2] {
    [base_types().1.grass, base_types().1.tree]
}

/// A new world's parameters as `tuning` has it, of `seed` if given:
/// side, camera and the generation's numbers -- what the server makes of
/// a window's sliders, never forced hot here.
fn parameters_of_tuning(seed: Option<u64>, tuning: &Tuning) -> CleanServerParameters {
    let numbers = (basegame::worldgen::world_generation::code(base_types().1).numbers_of_tuning)(tuning);
    let worldgen = numbers.into_iter().fold(CleanServerParameters::default(), |worldgen, (parameter_name, value)| worldgen.with(parameter_name, ParameterValue::Hex(value)));
    let mut parameters = CleanServerParameters::default().with(WORLDGEN, ParameterValue::Nested(worldgen)).with(SIDE, ParameterValue::Integer(tuning[utilities::tuning::WORLD_SIDE].round().max(0.0) as i64));
    if tuning[utilities::tuning::CAMERA_LOADS] >= 0.5 {
        parameters.insert(CAMERA_LOADS, ParameterValue::Nested(CleanServerParameters::default()));
    }
    match seed {
        Some(given_seed) => parameters.with(SEED, ParameterValue::Hex(given_seed)),
        None => parameters,
    }
}

/// The base game's types of a new world, frozen once, and their IDs: a
/// new world's are handed out in registration order, so every new
/// world's are these.
fn base_types() -> &'static (Arc<FrozenTypes>, BaseTypes) {
    static REGISTERED: OnceLock<(Arc<FrozenTypes>, BaseTypes)> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut type_registry = TypeRegistry::for_new_world();
        let base_types = basegame::default_types::register(&mut type_registry).expect("the base game's types registered");
        (Arc::new(type_registry.freeze().expect("frozen")), base_types)
    })
}

/// What a probe asks: a host on a thread of this process, or a server
/// over the pipe -- the program at a path run with `--pipe`.
pub enum ProbedServer {
    /// A host on a thread of this process.
    InProcess(Host),
    /// A server over the pipe: its process, and its stdin.
    OverPipe(Child, ChildStdin),
}

impl ProbedServer {
    /// A host of this process if `program` is empty, else the program at
    /// `program` run with `--pipe`; and where its frames come.
    pub fn start(program: &str) -> (Self, Receiver<ViewportFrame>) {
        if program.is_empty() {
            let (host, host_sent) = Host::start_on_thread(basegame::default_types::register_types);
            // The frames alone, the types sent once a world passed by.
            let (frame_sender, frame_receiver) = channel();
            std::thread::spawn(move || {
                while let Ok(sent) = host_sent.recv() {
                    if let server::host::HostSent::Frame(frame) = sent
                        && frame_sender.send(*frame).is_err()
                    {
                        return;
                    }
                }
            });
            return (Self::InProcess(host), frame_receiver);
        }
        let mut server_process = Command::new(program).arg("--pipe").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("the server started");
        let (to_server, mut from_server) = (server_process.stdin.take().expect("its stdin"), std::io::BufReader::new(server_process.stdout.take().expect("its stdout")));
        let (frame_sender, frame_receiver) = channel();
        std::thread::spawn(move || {
            while let Ok(Some(ServerMessage::Frame(frame))) = receive::<ServerMessage>(&mut from_server) {
                if frame_sender.send(*frame).is_err() {
                    return;
                }
            }
        });
        (Self::OverPipe(server_process, to_server), frame_receiver)
    }

    /// Tells it `message`: whether it was there to be told.
    fn tell(&mut self, message: ClientMessage) -> bool {
        match (self, message) {
            (Self::InProcess(host), ClientMessage::AskFrame(frame_ask)) => host.ask_frame(frame_ask),
            (Self::InProcess(host), ClientMessage::SetPace(pace)) => host.set_pace(pace),
            (Self::InProcess(host), ClientMessage::MakeWorld(seed, tuning)) => host.make_world(parameters_of_tuning(seed, &tuning), base_types().0.clone()),
            (Self::InProcess(_), _) => false,
            (Self::OverPipe(_, to_server), mut message) => send(&mut message, to_server).is_ok(),
        }
    }

    /// Asks for a frame.
    pub fn ask_frame(&mut self, frame_ask: FrameAsk) -> bool {
        self.tell(ClientMessage::AskFrame(frame_ask))
    }

    /// Holds it to so many ticks a second, or flat out.
    pub fn set_pace(&mut self, pace: Option<u32>) -> bool {
        self.tell(ClientMessage::SetPace(pace))
    }

    /// Makes a world from `seed`, or one drawn, as `tuning` says.
    pub fn make_world(&mut self, seed: Option<u64>, tuning: Tuning) -> bool {
        self.tell(ClientMessage::MakeWorld(seed, tuning))
    }
}

impl Drop for ProbedServer {
    fn drop(&mut self) {
        if let Self::OverPipe(server_process, _) = self {
            _ = server_process.kill();
            _ = server_process.wait();
        }
    }
}

/// How long a probe waits between two frames asked.
const BETWEEN_FRAMES: Duration = Duration::from_millis(20);

/// Frames a probe asks before it gives up on one that never comes.
const FRAMES_ASKED_AT_MOST: u32 = 30_000;

/// How long an answer is waited for: a world's first superchunks take a while to generate.
const ANSWER_WAITED_AT_MOST: Duration = Duration::from_secs(1800);

/// The seed `given_parameters` names, in hex: none if it is 0, a seed then drawn.
pub fn seed(given_parameters: &ProbeParameters) -> Result<Option<u64>, String> {
    let seed_text = given_parameters.argument_text(crate::SEED)?;
    let seed = utilities::seed::of_hex(&seed_text).ok_or_else(|| format!("`{seed_text}` is no seed in hex"))?;
    Ok((seed != 0).then_some(seed))
}

/// The whole of what the host answers an ask with: its frames taken
/// until the one that says no more is to come, the superchunks of all
/// of them in the last -- why not, if the host stops answering.
pub fn whole_answer(frame_receiver: &Receiver<ViewportFrame>) -> Result<ViewportFrame, String> {
    let mut cells = Vec::new();
    loop {
        let mut next_frame = frame_receiver.recv_timeout(ANSWER_WAITED_AT_MOST).map_err(|_| "the host never answered".to_string())?;
        cells.append(&mut next_frame.cells);
        if !next_frame.more_to_come {
            next_frame.cells = cells;
            return Ok(next_frame);
        }
    }
}

/// The first whole answer of `host` that `is_wanted` takes, every hot
/// superchunk of `viewport` asked for each time: why not, if none comes.
pub fn frame_that(host: &mut ProbedServer, frame_receiver: &Receiver<ViewportFrame>, viewport: Option<Viewport>, is_wanted: impl Fn(&ViewportFrame) -> bool) -> Result<ViewportFrame, String> {
    for _ in 0..FRAMES_ASKED_AT_MOST {
        if !host.ask_frame(FrameAsk { viewport, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None, planes: drawn_planes(), counted: counted().to_vec() }) {
            return Err("the host is gone".to_string());
        }
        let answered_frame = whole_answer(frame_receiver)?;
        if is_wanted(&answered_frame) {
            return Ok(answered_frame);
        }
        std::thread::sleep(BETWEEN_FRAMES);
    }
    Err("the host never answered with the frame waited for".to_string())
}
