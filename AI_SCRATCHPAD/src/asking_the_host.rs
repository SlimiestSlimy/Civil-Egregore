//! What the probes share: a host asked for frames until one answers
//! the question.

use server::host::frame::{Ask, Frame, Viewport};
use server::host::Host;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use utilities::commands::Given;

/// How long a probe waits between two frames asked.
const BETWEEN_FRAMES: Duration = Duration::from_millis(20);

/// Frames a probe asks before it gives up on one that never comes.
const FRAMES_ASKED_AT_MOST: u32 = 30_000;

/// How long an answer is waited for: a world's first superchunks take a while to generate.
const ANSWER_WAITED_AT_MOST: Duration = Duration::from_secs(1800);

/// The seed `given` names, in hex: none if it is 0, a seed then drawn.
pub fn seed(given: &Given) -> Result<Option<u64>, String> {
    let text = given.text(crate::SEED)?;
    let seed = utilities::seed::of_hex(text).ok_or_else(|| format!("`{text}` is no seed in hex"))?;
    Ok((seed != 0).then_some(seed))
}

/// The whole of what the host answers an ask with: its frames taken
/// until the one that says no more is to come, the superchunks of all
/// of them in the last -- why not, if the host stops answering.
pub fn whole_answer(frames: &Receiver<Frame>) -> Result<Frame, String> {
    let mut cells = Vec::new();
    loop {
        let mut frame = frames.recv_timeout(ANSWER_WAITED_AT_MOST).map_err(|_| "the host never answered".to_string())?;
        cells.append(&mut frame.cells);
        if !frame.more {
            frame.cells = cells;
            return Ok(frame);
        }
    }
}

/// The first whole answer of `host` that `wanted` takes, every hot
/// superchunk of `viewport` asked for each time: why not, if none comes.
pub fn frame_that(host: &Host, frames: &Receiver<Frame>, viewport: Option<Viewport>, wanted: impl Fn(&Frame) -> bool) -> Result<Frame, String> {
    for _ in 0..FRAMES_ASKED_AT_MOST {
        if !host.sync(Ask { viewport, detail: 0, skip: 0, most: u32::MAX, near: None }) {
            return Err("the host is gone".to_string());
        }
        let frame = whole_answer(frames)?;
        if wanted(&frame) {
            return Ok(frame);
        }
        std::thread::sleep(BETWEEN_FRAMES);
    }
    Err("the host never answered with the frame waited for".to_string())
}
