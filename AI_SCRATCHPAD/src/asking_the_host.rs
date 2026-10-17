//! What the probes share: a host asked for frames until one answers
//! the question.

use protocol::pipe::frame::{FrameAsk, ViewportFrame, Viewport};
use server::host::Host;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use crate::ProbeParameters;

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
pub fn frame_that(host: &Host, frame_receiver: &Receiver<ViewportFrame>, viewport: Option<Viewport>, is_wanted: impl Fn(&ViewportFrame) -> bool) -> Result<ViewportFrame, String> {
    for _ in 0..FRAMES_ASKED_AT_MOST {
        if !host.ask_frame(FrameAsk { viewport, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None }) {
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
