//! The server over the pipe, as a window starts it: the program run
//! with `--pipe`, asked over its stdin and answered on its stdout.

use protocol::pipe::frame::{FrameAsk, Viewport};
use protocol::pipe::messages::{ClientMessage, ServerMessage};
use std::process::{Command, Stdio};
use utilities::tuning::{default_tuning, FORCED_HOT, SHEEP_PER_SUPERCHUNK, WORLD_SIDE};
use utilities::wire::{receive, send};

/// A world made over the pipe is ticked and seen: the worlds listed,
/// then frames of the world made, until one shows its superchunks
/// hot; and the server ends, well, once its stdin is closed.
#[test]
fn a_world_made_over_the_pipe_is_answered_with_its_frames() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut server = Command::new(env!("CARGO_BIN_EXE_Civil_Egregore")).arg("--pipe").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("the server started");
    let (mut to_server, mut from_server) = (server.stdin.take().expect("its stdin"), server.stdout.take().expect("its stdout"));
    send(&mut ClientMessage::ListWorlds, &mut to_server).expect("sent");
    assert!(matches!(receive(&mut from_server).expect("read"), Some(ServerMessage::Worlds(_))), "the worlds listed");
    let mut tuning = default_tuning();
    (tuning[WORLD_SIDE], tuning[FORCED_HOT], tuning[SHEEP_PER_SUPERCHUNK]) = (2.0, 1.0, 3.0);
    send(&mut ClientMessage::MakeWorld(Some(seed), tuning), &mut to_server).expect("sent");
    let whole_world = Viewport { top_left_superchunk_cartesian: (0, 0), bottom_right_superchunk_cartesian: (u32::MAX, u32::MAX) };
    let mut frame_ask = ClientMessage::AskFrame(FrameAsk { viewport: Some(whole_world), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 4, seen_near: None });
    let mut frames_seen = 0;
    loop {
        send(&mut frame_ask, &mut to_server).expect("sent");
        let Some(ServerMessage::Frame(frame)) = receive(&mut from_server).expect("read") else {
            panic!("a frame answers a frame asked");
        };
        frames_seen += 1;
        if frame.seed == seed && frame.hot.len() == 4 {
            break;
        }
        assert!(frames_seen < 2_000, "the world made, 2 superchunks a side forced hot, is never seen whole");
    }
    drop(to_server);
    assert!(server.wait().expect("the server ended").success(), "the server ends well once the client is gone");
}
