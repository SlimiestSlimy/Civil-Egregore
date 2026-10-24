//! The server's end of its clients' connections: a host running a world,
//! and one client at a time connected to it over a socket on this
//! machine -- told and asked in the client's messages, answered in the
//! server's (`protocol::pipe::messages`), at the one port
//! ([`SERVER_PORT`]). Where it listens is said once, as it starts
//! listening: what the client that started it waits for before it
//! connects. Told to ([`CLOSE_IF_NO_CLIENTS`]), the server closes once
//! no client has been connected for [`CLIENT_WAIT`]; else it stays
//! open, clients coming and going. It runs the world its parameters say
//! from the start, if they say one; else none until a client makes or
//! opens one.

use crate::host::{Host, HostSent};
use crate::params::{CLOSE_IF_NO_CLIENTS, SEED, SIDE, WORLD};
use crate::worlds_in;
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use protocol::pipe::SERVER_PORT;
use protocol::server_parameters::CleanServerParameters;
use protocol::world_types::{RegisterTypes, TYPES_FILE};
use std::io::BufReader;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// How long a server told to close if no clients waits for one, as it
/// starts and whenever its client leaves.
pub const CLIENT_WAIT: Duration = Duration::from_secs(30);
use utilities::wire::{receive, send};

/// Where what the server says goes: the client connected, if one is,
/// and the running world's types -- what a client connecting later is
/// told first. One lock for both, so a client connecting never misses
/// a world's types, nor is told them twice.
#[derive(Default)]
struct Outgoing {
    /// The client connected, if one is.
    client: Option<TcpStream>,
    /// The running world's types, as last sent: none before a world runs.
    world_types: Option<(u64, String)>,
}

impl Outgoing {
    /// Sends `message` to the client connected, whole; one that cannot be
    /// sent to is let go.
    fn send_to_client(&mut self, mut message: ServerMessage) {
        if let Some(client) = &mut self.client
            && send(&mut message, client).is_err()
        {
            self.client = None;
        }
    }
}

/// Serves clients, one at a time -- until none has been connected for
/// [`CLIENT_WAIT`], if told to close then, else until the process is
/// stopped:
/// their asks done by a host of this process's own, the frames sent as
/// the host makes them, every other ask answered in the order asked on
/// a thread of its own -- a map takes long, and the frames go on
/// meanwhile. Each world's type registry frozen by `register_types`
/// before anything of it. The world `clean_parameters` say runs from
/// the start, if they say one -- the one in their [`WORLD`] folder, or a
/// new one of their seed and side made there; or why it does not.
pub fn serve_clients(clean_parameters: &CleanServerParameters, register_types: RegisterTypes) -> Result<String, String> {
    let close_if_no_clients = clean_parameters.nested(CLOSE_IF_NO_CLIENTS).is_some();
    // The port first: a server that cannot listen starts nothing.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, SERVER_PORT)).map_err(|bind_error| format!("port {SERVER_PORT} not listened on -- another server running? {bind_error}"))?;
    let (host, host_sent) = Host::start_on_thread(register_types);
    if clean_parameters.each().any(|(parameter_name, _)| [SEED, SIDE, WORLD].contains(&parameter_name)) {
        let folder = utilities::settings::world(clean_parameters.text(WORLD).ok_or(format!("a world is made in a folder: --{WORLD}"))?);
        let world = if folder.join(TYPES_FILE).exists() {
            let frozen_type_registry = crate::read_types(&folder, register_types).map_err(|read_error| read_error.to_string())?;
            crate::run_world(&folder, &Arc::new(frozen_type_registry), clean_parameters)
        } else {
            crate::make_world_of(&folder, clean_parameters, register_types)
        };
        let world = world.map_err(|why_not| why_not.to_string())?;
        println!("seed {}, at tick {}: running", utilities::seed::seed_as_hex(world.seed()), world.entity_store().current_tick());
        _ = host.run_world(world);
    }
    // The one line said: the client that started the server reads it to connect.
    println!("listening on {}", listener.local_addr().map_err(|address_error| address_error.to_string())?);
    let outgoing = Arc::new(Mutex::new(Outgoing::default()));
    let frames_outgoing = Arc::clone(&outgoing);
    thread::Builder::new()
        .name("frames out".to_string())
        .spawn(move || {
            for sent in host_sent {
                let mut outgoing = frames_outgoing.lock().expect("the client's connection");
                match sent {
                    HostSent::WorldRuns(world, frozen_type_registry) => {
                        let types_text = frozen_type_registry.to_text();
                        outgoing.world_types = Some((world, types_text.clone()));
                        outgoing.send_to_client(ServerMessage::WorldTypes(world, types_text));
                    }
                    HostSent::Frame(frame) => outgoing.send_to_client(ServerMessage::Frame(frame)),
                }
            }
        })
        .map_err(|spawn_error| spawn_error.to_string())?;
    let (ask_sender, asks) = channel();
    let (answers_outgoing, calls_host) = (Arc::clone(&outgoing), host.clone());
    thread::Builder::new()
        .name("answers out".to_string())
        .spawn(move || {
            for ask in asks {
                let answer = match ask {
                    ClientMessage::ListWorlds => ServerMessage::Worlds(worlds_in(&utilities::settings::worlds())),
                    ClientMessage::Call(call, arguments) => match calls_host.call(call, arguments) {
                        Ok(answer) => ServerMessage::Answered(answer),
                        Err(why_refused) => ServerMessage::CallRefused(why_refused),
                    },
                    _ => continue,
                };
                answers_outgoing.lock().expect("the client's connection").send_to_client(answer);
            }
        })
        .map_err(|spawn_error| spawn_error.to_string())?;
    // Clients taken in as they connect, served one at a time: the next waits until the one connected leaves.
    let (client_sender, clients) = channel();
    thread::Builder::new()
        .name("clients in".to_string())
        .spawn(move || {
            for client in listener.incoming().flatten() {
                if client_sender.send(client).is_err() {
                    return;
                }
            }
        })
        .map_err(|spawn_error| spawn_error.to_string())?;
    loop {
        let client = match if close_if_no_clients { clients.recv_timeout(CLIENT_WAIT) } else { clients.recv().map_err(|_| RecvTimeoutError::Disconnected) } {
            Ok(client) => client,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return Ok(String::new()),
        };
        _ = client.set_nodelay(true);
        let Ok(client_reader) = client.try_clone() else { continue };
        {
            let mut outgoing = outgoing.lock().expect("the client's connection");
            outgoing.client = Some(client);
            // Joining a world already running: its types first, as every client is told them.
            if let Some((world, types_text)) = outgoing.world_types.clone() {
                outgoing.send_to_client(ServerMessage::WorldTypes(world, types_text));
            }
        }
        serve_one_client(client_reader, &host, &ask_sender);
        outgoing.lock().expect("the client's connection").client = None;
        // Left by its client, the world goes on as it was before one came.
        _ = host.set_paused(false);
    }
}

/// Reads `client`'s messages until it leaves, each done by `host` or
/// sent on to be answered in the order asked.
fn serve_one_client(client: TcpStream, host: &Host, ask_sender: &std::sync::mpsc::Sender<ClientMessage>) {
    let mut client = BufReader::new(client);
    while let Ok(Some(message)) = receive::<ClientMessage>(&mut client) {
        // The host gone is the process ending: nothing to tell it.
        match message {
            ClientMessage::AskFrame(frame_ask) => _ = host.ask_frame(frame_ask),
            ClientMessage::SetPaused(paused) => _ = host.set_paused(paused),
            ClientMessage::SetPace(pace) => _ = host.set_pace(pace),
            ClientMessage::MakeWorld(parameters) => _ = host.make_world(parameters),
            ClientMessage::OpenWorld(world_name) => _ = host.open_world(world_name),
            ClientMessage::Save(world_name) => _ = host.save_world(world_name),
            // Answered, in the order asked: a client matches each answer to its ask by that order.
            ask @ (ClientMessage::ListWorlds | ClientMessage::Call(..)) => _ = ask_sender.send(ask),
        }
    }
}
