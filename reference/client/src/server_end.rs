//! The server, as the client holds it: the client's own program run
//! again with `--close_if_no_clients`, a process of its own, connected to
//! over a socket on this machine at the port it says as it starts --
//! told, asked and answering in the messages of `protocol::pipe::messages`.
//! Frames come as the server sends them; every other answer comes in
//! the order asked, and goes to whoever asked.

use protocol::pipe::frame::{FrameAsk, ViewportFrame};
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use protocol::server_parameters::CleanServerParameters;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
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
    /// A frame asked for: boxed, the types' message small beside it.
    Frame(Box<ViewportFrame>),
}

/// Where the server is told and asked: cloned for each thread that asks.
#[derive(Clone)]
pub struct ServerLink {
    /// The connection to the server, and who waits for an answer: one
    /// lock for both, so the order they wait in is the order the server
    /// reads in.
    writing: Arc<Mutex<Writing>>,
}

/// What telling and asking the server goes through.
struct Writing {
    /// The connection to the server, as written to.
    to_server: TcpStream,
    /// Who waits for an answer, in the order asked.
    answers_awaited: VecDeque<Sender<ServerMessage>>,
}

impl ServerEnd {
    /// Starts the server: this program again, with `--close_if_no_clients`,
    /// connected to at the address it says. What it shows -- each world's
    /// types, and its frames -- comes back on the receiver.
    pub fn start() -> (Self, Receiver<Shown>) {
        let program = std::env::current_exe().expect("the program's own path");
        let mut server_process = Command::new(program).arg("--close_if_no_clients").stdin(Stdio::null()).stdout(Stdio::piped()).spawn().expect("the server started");
        let to_server = connect_to(&mut server_process);
        let from_server = to_server.try_clone().expect("the connection, to read from");
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
                            if frame_sender.send(Shown::Frame(frame)).is_err() {
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

/// The connection to `server_process`, at the address the line it says
/// as it starts listening names (`listening on 127.0.0.1:<port>`).
fn connect_to(server_process: &mut Child) -> TcpStream {
    let mut said = BufReader::new(server_process.stdout.take().expect("the server's stdout"));
    let mut said_line = String::new();
    while said.read_line(&mut said_line).expect("the server's stdout") > 0 {
        if let Some(address) = said_line.trim().strip_prefix("listening on ") {
            let to_server = TcpStream::connect(address).expect("connected to the server");
            _ = to_server.set_nodelay(true);
            return to_server;
        }
        said_line.clear();
    }
    panic!("the server ended before it said where it listens")
}

impl Drop for ServerEnd {
    fn drop(&mut self) {
        // Its connection is held by every link still about: ended outright.
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

    /// Calls the function whose ID is `call` -- a type registered as a
    /// client call -- with `arguments`, and waits for its answer, or why
    /// it is refused: none if the server is gone.
    pub fn call(&self, call: u64, arguments: Vec<u64>) -> Option<Result<Vec<u64>, String>> {
        self.ask(ClientMessage::Call(call, arguments))?.into_answer()
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

    /// Makes a world as `parameters` say -- the server's own, as a
    /// command line gives them -- and runs it in place of any run.
    pub fn make_world(&self, parameters: CleanServerParameters) -> bool {
        self.tell(ClientMessage::MakeWorld(parameters))
    }

    /// Runs the world named `world_name` in the worlds' folder in place of any run.
    pub fn open_world(&self, world_name: String) -> bool {
        self.tell(ClientMessage::OpenWorld(world_name))
    }

    /// Saves the world run in its folder.
    pub fn save_world(&self) -> bool {
        self.tell(ClientMessage::Save)
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
