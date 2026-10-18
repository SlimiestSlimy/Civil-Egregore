//! The server, as the client holds it: the client's own program run
//! again with `--pipe`, a process of its own, told and asked over its
//! stdin and answering on its stdout (`protocol::pipe::messages`).
//! Frames come as the server sends them; every other answer comes in
//! the order asked, and goes to whoever asked.

use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use protocol::pipe::terrain::{AsksTerrain, Height, HeightsAsk, MapAnswer, MapAsk};
use std::collections::VecDeque;
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use utilities::tuning::Tuning;
use utilities::wire::{receive, send};

/// The server's process, and where it is told and asked. Dropped, the
/// process ends.
pub struct ServerEnd {
    /// Where it is told and asked, by any thread.
    pub link: ServerLink,
    /// Its process.
    server_process: Child,
}

/// What the server shows the window, in the order it sends it: each
/// world's types, once, before any frame of it; and the frames.
pub enum Shown {
    /// A world runs, the `n`-th the server has run: its types, as its save
    /// holds them.
    WorldTypes(u64, String),
    /// A frame asked for.
    Frame(ViewportFrame),
}

/// Where the server is told and asked: cloned for each thread that asks.
#[derive(Clone)]
pub struct ServerLink {
    /// The server's stdin, and who waits for an answer: one lock for
    /// both, so the order they wait in is the order the server reads in.
    writing: Arc<Mutex<Writing>>,
}

/// What telling and asking the server goes through.
struct Writing {
    /// The server's stdin.
    to_server: ChildStdin,
    /// Who waits for an answer, in the order asked.
    answers_awaited: VecDeque<Sender<ServerMessage>>,
}

impl ServerEnd {
    /// Starts the server: this program again, with `--pipe`. What it
    /// shows -- each world's types, and its frames -- comes back on the
    /// receiver.
    pub fn start() -> (Self, Receiver<Shown>) {
        let program = std::env::current_exe().expect("the program's own path");
        let mut server_process = Command::new(program).arg("--pipe").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("the server started");
        let (to_server, from_server) = (server_process.stdin.take().expect("the server's stdin"), server_process.stdout.take().expect("the server's stdout"));
        let link = ServerLink { writing: Arc::new(Mutex::new(Writing { to_server, answers_awaited: VecDeque::new() })) };
        let (frame_sender, frame_receiver) = channel();
        let reading_link = link.clone();
        thread::Builder::new()
            .name("from the server".to_string())
            .spawn(move || {
                let mut from_server = BufReader::new(from_server);
                while let Ok(Some(message)) = receive::<ServerMessage>(&mut from_server) {
                    match message {
                        ServerMessage::WorldTypes(world, types_text) => {
                            if frame_sender.send(Shown::WorldTypes(world, types_text)).is_err() {
                                return;
                            }
                        }
                        ServerMessage::Frame(frame) => {
                            if frame_sender.send(Shown::Frame(*frame)).is_err() {
                                return;
                            }
                        }
                        answer => {
                            let awaiting = reading_link.writing.lock().expect("the server's link").answers_awaited.pop_front();
                            // An asker gone before its answer is nothing to tell anyone.
                            awaiting.map(|answer_sender| answer_sender.send(answer));
                        }
                    }
                }
            })
            .expect("a thread for the server's answers");
        (Self { link, server_process }, frame_receiver)
    }
}

impl Drop for ServerEnd {
    fn drop(&mut self) {
        // Its stdin is held by every link still about: ended outright.
        _ = self.server_process.kill();
        _ = self.server_process.wait();
    }
}

impl ServerLink {
    /// Tells the server `message`: whether it was there to be told.
    fn tell(&self, mut message: ClientMessage) -> bool {
        send(&mut message, &mut self.writing.lock().expect("the server's link").to_server).is_ok()
    }

    /// Asks the server `message`, and waits for its answer: none if it
    /// is gone.
    fn ask(&self, mut message: ClientMessage) -> Option<ServerMessage> {
        let (answer_sender, answer_receiver) = channel();
        {
            let mut writing = self.writing.lock().expect("the server's link");
            writing.answers_awaited.push_back(answer_sender);
            send(&mut message, &mut writing.to_server).ok()?;
        }
        answer_receiver.recv().ok()
    }

    /// Asks for some of the viewport's superchunks: answered with a
    /// frame -- unless no world runs, when nothing is.
    pub fn ask_frame(&self, frame_ask: FrameAsk) -> bool {
        self.tell(ClientMessage::AskFrame(frame_ask))
    }

    /// Stops ticking, or goes on.
    pub fn set_paused(&self, ticking_paused: bool) -> bool {
        self.tell(ClientMessage::SetPaused(ticking_paused))
    }

    /// Ticks so many times a second, or flat out.
    pub fn set_pace(&self, pace_held_to: Option<u32>) -> bool {
        self.tell(ClientMessage::SetPace(pace_held_to))
    }

    /// Makes a world from `seed`, or one drawn, as `tuning` says, and runs
    /// it in place of any run.
    pub fn make_world(&self, seed: Option<u64>, tuning: Tuning) -> bool {
        self.tell(ClientMessage::MakeWorld(seed, tuning))
    }

    /// Runs the world named `world_name` in the worlds' folder in place of any run.
    pub fn open_world(&self, world_name: String) -> bool {
        self.tell(ClientMessage::OpenWorld(world_name))
    }

    /// Saves the world run under `world_name` in the worlds' folder.
    pub fn save_world(&self, world_name: String) -> bool {
        self.tell(ClientMessage::Save(world_name))
    }

    /// Makes the world run again from its start, generated as `tuning` says.
    pub fn reset(&self, tuning: Tuning) -> bool {
        self.tell(ClientMessage::Reset(tuning))
    }

    /// The names of the worlds in the worlds' folder: none if the
    /// server is gone.
    pub fn list_worlds(&self) -> Vec<String> {
        match self.ask(ClientMessage::ListWorlds) {
            Some(ServerMessage::Worlds(world_names)) => world_names,
            _ => Vec::new(),
        }
    }
}

impl AsksTerrain for ServerLink {
    fn heights(&self, heights_ask: HeightsAsk) -> Option<Vec<Height>> {
        match self.ask(ClientMessage::AskHeights(heights_ask))? {
            ServerMessage::Heights(heights) => heights,
            _ => None,
        }
    }

    fn map(&self, map_ask: MapAsk) -> Option<MapAnswer> {
        match self.ask(ClientMessage::AskMap(map_ask))? {
            ServerMessage::Map(map_answer) => map_answer,
            _ => None,
        }
    }
}
