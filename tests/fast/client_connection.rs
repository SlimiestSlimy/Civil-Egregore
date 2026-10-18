//! The server and its client, connected over a socket on this machine:
//! the program started as a window starts it, to close if no clients,
//! and started to stay open, clients coming and going (its closing
//! once none has been connected a while: the complete tier). One server a machine at a time
//! (`protocol::pipe::SERVER_PORT`): the tests here take turns, and fail
//! if another server -- a game's -- is running.

use crate::worlds::base;
use protocol::pipe::frame::{FrameAsk, Viewport};
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use utilities::tuning::{default_tuning, FORCED_HOT, SHEEP_PER_SUPERCHUNK, WORLD_SIDE};
use utilities::wire::{receive, send};

/// Held while a test's server runs: one at a time on the one port.
static ONE_SERVER: Mutex<()> = Mutex::new(());

/// The program started with `arguments`, its output read until it says
/// where it listens.
fn server_started(arguments: &[&str]) -> (Child, String) {
    let mut server = Command::new(env!("CARGO_BIN_EXE_Civil_Egregore")).args(arguments).stdin(Stdio::null()).stdout(Stdio::piped()).spawn().expect("the server started");
    let said = BufReader::new(server.stdout.take().expect("its stdout")).lines();
    let address = said.map_while(Result::ok).find_map(|line| line.strip_prefix("listening on ").map(str::to_string)).expect("where it listens, said");
    (server, address)
}

/// A frame ask of every superchunk, the grass plane asked for and counted.
fn whole_world_ask() -> ClientMessage {
    let whole_world = Viewport { top_left_superchunk_cartesian: (0, 0), bottom_right_superchunk_cartesian: (u32::MAX, u32::MAX) };
    ClientMessage::AskFrame(FrameAsk { viewport: Some(whole_world), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 4, seen_near: None, planes: vec![base().grass], counted: vec![base().grass] })
}

/// A server started as a window starts it runs no world until its
/// client makes one: then the world made is ticked and seen -- the
/// worlds listed, then frames of the world made, until one shows its
/// superchunks hot, each with the plane asked for and the layer
/// counted, the world's types sent once before them; and the server
/// waits a while for another client once its client is gone.
#[test]
fn a_world_made_by_a_client_is_answered_with_its_frames() {
    let _one_server = ONE_SERVER.lock().unwrap_or_else(PoisonError::into_inner);
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut server, address) = server_started(&["--close_if_no_clients"]);
    let mut to_server = TcpStream::connect(&address).expect("connected");
    let mut from_server = BufReader::new(to_server.try_clone().expect("to read from"));
    send(&mut ClientMessage::ListWorlds, &mut to_server).expect("sent");
    assert!(matches!(receive(&mut from_server).expect("read"), Some(ServerMessage::Worlds(_))), "the worlds listed");
    let mut tuning = default_tuning();
    (tuning[WORLD_SIDE], tuning[FORCED_HOT], tuning[SHEEP_PER_SUPERCHUNK]) = (2.0, 1.0, 3.0);
    send(&mut ClientMessage::MakeWorld(Some(seed), tuning), &mut to_server).expect("sent");
    let mut frame_ask = whole_world_ask();
    let (mut frames_seen, mut types_seen) = (0, 0);
    loop {
        send(&mut frame_ask, &mut to_server).expect("sent");
        // The world's types come once, before its first frame.
        let frame = loop {
            match receive(&mut from_server).expect("read") {
                Some(ServerMessage::Frame(frame)) => break frame,
                Some(ServerMessage::WorldTypes(..)) => types_seen += 1,
                _ => panic!("a frame answers a frame asked"),
            }
        };
        frames_seen += 1;
        if frame.seed == seed && frame.hot.len() == 4 {
            assert!(frame.counts.len() == 1 && frame.counts[0].0 == base().grass, "the layer counted, and no other");
            assert!(frame.cells.iter().all(|cells| cells.planes.iter().all(|(layer_type, _)| *layer_type == base().grass)), "the plane asked for, and no other");
            assert_eq!(types_seen, 1, "the world's types sent once");
            break;
        }
        assert!(frames_seen < 2_000, "the world made, 2 superchunks a side forced hot, is never seen whole");
    }
    drop((to_server, from_server));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(server.try_wait().expect("asked").is_none(), "the server waits for another client a while");
    server.kill().expect("stopped");
    _ = server.wait();
}

/// A server left open runs the world its flags say from the start, and
/// goes on running it as clients come and go: each client, joining
/// late, is told the running world's types first, then answered with
/// its frames -- of the seed the flags gave; a call of what no client
/// may call is refused, and the asks after it answered in order.
#[test]
fn a_server_tells_each_client_joining_its_world_the_world_s_types_first() {
    let _one_server = ONE_SERVER.lock().unwrap_or_else(PoisonError::into_inner);
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut server, address) = server_started(&["--seed", &format!("{seed:#x}"), "--side", "2"]);
    let mut worlds_told = Vec::new();
    for client in 0..2 {
        let mut to_server = TcpStream::connect(&address).expect("connected");
        let mut from_server = BufReader::new(to_server.try_clone().expect("to read from"));
        send(&mut whole_world_ask(), &mut to_server).expect("sent");
        let Some(ServerMessage::WorldTypes(world, _)) = receive(&mut from_server).expect("read") else { panic!("client {client}: the world's types not told first") };
        worlds_told.push(world);
        let Some(ServerMessage::Frame(frame)) = receive(&mut from_server).expect("read") else { panic!("client {client}: a frame answers a frame asked") };
        assert_eq!(frame.seed, seed, "client {client}: the world the flags said");
        // A call of what no client may call -- a plane's ID -- refused, and the asks after it answered in order.
        send(&mut ClientMessage::Call(base().grass.0, vec![1, 2]), &mut to_server).expect("sent");
        send(&mut ClientMessage::ListWorlds, &mut to_server).expect("sent");
        let mut answers = std::iter::from_fn(|| loop {
            match receive(&mut from_server).expect("read") {
                Some(ServerMessage::Frame(_) | ServerMessage::WorldTypes(..)) => continue,
                answer => return answer,
            }
        });
        assert!(matches!(answers.next(), Some(ServerMessage::CallRefused(why_refused)) if why_refused.contains("no function a client may call")), "client {client}: a plane is no call");
        assert!(matches!(answers.next(), Some(ServerMessage::Worlds(_))), "client {client}: the next ask answered after it");
        drop((to_server, from_server));
        // A moment for the server to see its client gone: it must still be running after.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(server.try_wait().expect("asked").is_none(), "client {client} gone, the server left open ended");
    }
    assert_eq!(worlds_told[0], worlds_told[1], "the same world, told to each");
    server.kill().expect("stopped");
    _ = server.wait();
}
