//! The server's end of the pipe: a host serving the client that started
//! this process, the client's messages read from stdin and answered on
//! stdout (`protocol::pipe::messages`), until the client closes it.

use crate::host::{Host, HostSent};
use crate::worlds_in;
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use protocol::pipe::terrain::AsksTerrain;
use protocol::world_types::{self, GameTypes};
use std::io;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use utilities::wire::{receive, send};

/// Serves the client at the other end of stdin and stdout until it
/// closes stdin: its asks done by a host of this process's own, the
/// frames sent as the host makes them, every other ask answered in the
/// order asked on a thread of its own -- a map takes long, and the
/// frames go on meanwhile. Each world's types `game_types`, registered
/// and frozen before anything of it: a new world's here, as it is
/// asked for, its generation then asked how to start it; an opened
/// world's by the host, as its save gave them.
pub fn serve_the_pipe(game_types: GameTypes) -> Result<String, String> {
    let (host, host_sent) = Host::start_on_thread(game_types);
    thread::Builder::new()
        .name("frames out".to_string())
        .spawn(move || {
            for sent in host_sent {
                let message = match sent {
                    HostSent::WorldRuns(world, frozen_types) => ServerMessage::WorldTypes(world, frozen_types.to_text()),
                    HostSent::Frame(frame) => ServerMessage::Frame(frame),
                };
                if send_out(message).is_err() {
                    return;
                }
            }
        })
        .map_err(|spawn_error| spawn_error.to_string())?;
    let (ask_sender, asks) = channel();
    let terrain = host.terrain();
    thread::Builder::new()
        .name("answers out".to_string())
        .spawn(move || {
            for ask in asks {
                let answer = match ask {
                    ClientMessage::AskHeights(heights_ask) => ServerMessage::Heights(terrain.heights(heights_ask)),
                    ClientMessage::AskMap(map_ask) => ServerMessage::Map(terrain.map(map_ask)),
                    ClientMessage::ListWorlds => ServerMessage::Worlds(worlds_in(&utilities::settings::worlds())),
                    _ => continue,
                };
                if send_out(answer).is_err() {
                    return;
                }
            }
        })
        .map_err(|spawn_error| spawn_error.to_string())?;
    let mut client_end = io::stdin().lock();
    while let Some(message) = receive::<ClientMessage>(&mut client_end).map_err(|read_error| format!("the client's message: {read_error}"))? {
        // The host gone is the process ending: nothing to tell it.
        match message {
            ClientMessage::AskFrame(frame_ask) => _ = host.ask_frame(frame_ask),
            ClientMessage::SetPaused(paused) => _ = host.set_paused(paused),
            ClientMessage::SetPace(pace) => _ = host.set_pace(pace),
            ClientMessage::MakeWorld(seed, tuning) => {
                let frozen_types = Arc::new(world_types::of_a_new_world(game_types)?);
                let worldgen = crate::simulation::worldgen_of(&frozen_types)?;
                _ = host.make_world(crate::world_start::parameters_of_tuning(seed, &tuning, &worldgen), frozen_types);
            }
            ClientMessage::OpenWorld(world_name) => _ = host.open_world(world_name),
            ClientMessage::Save(world_name) => _ = host.save_world(world_name),
            ClientMessage::Reset(tuning) => _ = host.reset(&tuning),
            // Answered, in the order asked: a client matches each answer to its ask by that order.
            ask @ (ClientMessage::AskHeights(_) | ClientMessage::AskMap(_) | ClientMessage::ListWorlds) => _ = ask_sender.send(ask),
        }
    }
    Ok(String::new())
}

/// Sends `message` on stdout whole, stdout held the while: messages of
/// several threads never interleave.
fn send_out(mut message: ServerMessage) -> io::Result<()> {
    send(&mut message, &mut io::stdout().lock())
}
