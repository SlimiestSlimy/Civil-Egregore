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

/// The seed `given` names, in hex: none if it is 0, a seed then drawn.
pub fn seed(given: &Given) -> Result<Option<u64>, String> {
    let text = given.text(crate::SEED)?;
    let seed = utilities::seed::of_hex(text).ok_or_else(|| format!("`{text}` is no seed in hex"))?;
    Ok((seed != 0).then_some(seed))
}

/// The first frame `host` answers with that `wanted` takes, every hot
/// superchunk of `viewport` asked for each time: why not, if none comes.
pub fn frame_that(host: &Host, frames: &Receiver<Frame>, viewport: Option<Viewport>, wanted: impl Fn(&Frame) -> bool) -> Result<Frame, String> {
    for _ in 0..FRAMES_ASKED_AT_MOST {
        if !host.sync(Ask { viewport, detail: 0, skip: 0, most: u32::MAX, near: None }) {
            return Err("the host is gone".to_string());
        }
        if let Ok(frame) = frames.recv_timeout(Duration::from_secs(1))
            && wanted(&frame)
        {
            return Ok(frame);
        }
        std::thread::sleep(BETWEEN_FRAMES);
    }
    Err("the host never answered with the frame waited for".to_string())
}
