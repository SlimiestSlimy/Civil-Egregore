//! The wire between a ticker and the window showing its worlds: what the
//! window asks and the frames answered, as words of 64 bits over a
//! local socket -- so a ticker needs nothing of a window but this, and a
//! window nothing of a ticker's process (`docs/server.md`, "A world
//! shown").

use super::frame::{Ask, Cells, Frame, Near, Viewport, Water};
use super::terrain::Levels;
use super::Request;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::Arc;
use worldgen::Generation;

/// The most words a message is read as: more is no message of ours.
const MOST_WORDS: u64 = 1 << 28;

/// Sends `words` as one message: how many, then each, low byte first.
pub(crate) fn send(to: &mut impl Write, words: &[u64]) -> io::Result<()> {
    let mut bytes = Vec::with_capacity((words.len() + 1) * 8);
    bytes.extend_from_slice(&(words.len() as u64).to_le_bytes());
    words.iter().for_each(|word| bytes.extend_from_slice(&word.to_le_bytes()));
    to.write_all(&bytes)?;
    to.flush()
}

/// Reads one message [`send`] sent: its words.
pub(crate) fn receive(from: &mut impl Read) -> io::Result<Vec<u64>> {
    let mut word = [0u8; 8];
    from.read_exact(&mut word)?;
    let count = u64::from_le_bytes(word);
    if count > MOST_WORDS {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "a message longer than any sent"));
    }
    let mut bytes = vec![0u8; count as usize * 8];
    from.read_exact(&mut bytes)?;
    Ok(bytes.as_chunks::<8>().0.iter().map(|word| u64::from_le_bytes(*word)).collect())
}

/// A message being written, word after word.
#[derive(Default)]
pub(crate) struct Written(pub(crate) Vec<u64>);

impl Written {
    /// One number.
    fn number(&mut self, number: u64) {
        self.0.push(number);
    }

    /// A pair of them, in one word.
    fn pair(&mut self, (first, second): (u32, u32)) {
        self.0.push(u64::from(first) << 32 | u64::from(second));
    }

    /// Words, how many before them.
    fn words(&mut self, words: &[u64]) {
        self.0.push(words.len() as u64);
        self.0.extend_from_slice(words);
    }

    /// A text, its bytes eight a word.
    fn text(&mut self, text: &str) {
        self.0.push(text.len() as u64);
        self.0.extend(text.as_bytes().chunks(8).map(|bytes| bytes.iter().rev().fold(0, |word, &byte| word << 8 | u64::from(byte))));
    }

    /// Something there may be none of: whether there is, then it.
    fn maybe<T>(&mut self, maybe: Option<T>, mut written: impl FnMut(&mut Self, T)) {
        self.0.push(u64::from(maybe.is_some()));
        if let Some(there) = maybe {
            written(self, there);
        }
    }
}

/// A message being read, word after word: `None` from any read past
/// its end, so a message cut short is no message.
pub(crate) struct Read64<'a> {
    /// Its words.
    words: &'a [u64],
    /// The next to read.
    at: usize,
}

impl<'a> Read64<'a> {
    /// `words`, from the first.
    pub(crate) fn of(words: &'a [u64]) -> Self {
        Self { words, at: 0 }
    }

    /// One number.
    fn number(&mut self) -> Option<u64> {
        self.at += 1;
        self.words.get(self.at - 1).copied()
    }

    /// A pair of them.
    fn pair(&mut self) -> Option<(u32, u32)> {
        self.number().map(|word| ((word >> 32) as u32, word as u32))
    }

    /// Words, how many before them.
    fn words(&mut self) -> Option<Vec<u64>> {
        let count = self.number()? as usize;
        let words = self.words.get(self.at..self.at.checked_add(count)?)?;
        self.at += count;
        Some(words.to_vec())
    }

    /// A text.
    fn text(&mut self) -> Option<String> {
        let bytes = self.number()? as usize;
        let words = self.words.get(self.at..self.at.checked_add(bytes.div_ceil(8))?)?;
        self.at += words.len();
        String::from_utf8(words.iter().flat_map(|word| word.to_le_bytes()).take(bytes).collect()).ok()
    }

    /// Something there may be none of.
    fn maybe<T>(&mut self, read: impl FnOnce(&mut Self) -> Option<T>) -> Option<Option<T>> {
        match self.number()? {
            0 => Some(None),
            _ => read(self).map(Some),
        }
    }
}

/// A viewport's two corners.
fn viewport_written(written: &mut Written, viewport: Viewport) {
    written.pair(viewport.first);
    written.pair(viewport.last);
}

/// A viewport read.
fn viewport_read(read: &mut Read64) -> Option<Viewport> {
    Some(Viewport { first: read.pair()?, last: read.pair()? })
}

/// The cells seen from near, written.
fn near_written(written: &mut Written, near: Near) {
    written.pair(near.first);
    written.pair(near.size);
    written.number(u64::from(near.pixels_a_cell));
}

/// The cells seen from near, read.
fn near_read(read: &mut Read64) -> Option<Near> {
    Some(Near { first: read.pair()?, size: read.pair()?, pixels_a_cell: read.number()? as u32 })
}

/// What a window asks of a ticker, as a message: none of what only a
/// host that owns its world can do -- make one, open one, save one.
pub(crate) fn request_written(request: &Request) -> Option<Vec<u64>> {
    let mut written = Written::default();
    match request {
        Request::Sync(ask) => {
            written.number(0);
            written.maybe(ask.viewport, viewport_written);
            written.pair((ask.detail, ask.skip));
            written.number(u64::from(ask.most));
            written.maybe(ask.near, near_written);
        }
        Request::Pause(paused) => written.pair((1, u32::from(*paused))),
        Request::Pace(pace) => {
            written.number(2);
            written.maybe(*pace, |written, pace| written.number(u64::from(pace)));
        }
        Request::New(_) | Request::Open(_) | Request::Save(_) | Request::Reset(_) => return None,
    }
    Some(written.0)
}

/// What [`request_written`] wrote.
pub(crate) fn request_read(words: &[u64]) -> Option<Request> {
    let mut read = Read64::of(words);
    Some(match read.number()? {
        0 => {
            let viewport = read.maybe(viewport_read)?;
            let ((detail, skip), most) = (read.pair()?, read.number()? as u32);
            Request::Sync(Ask { viewport, detail, skip, most, near: read.maybe(near_read)? })
        }
        2 => Request::Pace(read.maybe(|read| read.number().map(|pace| pace as u32))?),
        pause => Request::Pause(pause & 1 == 1),
    })
}

/// A frame as a message. A superchunk's water goes with its heights,
/// the once they are sent: the window keeps it from then.
pub(crate) fn frame_written(frame: &Frame) -> Vec<u64> {
    let mut written = Written::default();
    [frame.world, frame.seed, frame.tick, frame.ticks_a_second.to_bits(), frame.sheep as u64, frame.grass, frame.trees, frame.sync_seconds.to_bits(), frame.sync_share.to_bits(), u64::from(frame.detail), u64::from(frame.more)].into_iter().for_each(|number| written.number(number));
    let numbers = frame.generation.numbers();
    written.number(numbers.len() as u64);
    for (name, number) in &numbers {
        written.text(name);
        written.number(*number);
    }
    written.maybe(frame.side, |written, side| written.number(u64::from(side)));
    written.maybe(frame.viewport, viewport_written);
    written.maybe(frame.near, near_written);
    written.maybe(frame.named.as_deref(), Written::text);
    written.maybe(frame.said.as_deref(), Written::text);
    written.number(frame.hot.len() as u64);
    frame.hot.iter().for_each(|&at| written.pair(at));
    written.number(frame.cells.len() as u64);
    for cells in &frame.cells {
        written.pair(cells.at);
        written.pair(cells.top_left);
        [&cells.grass, &cells.heights, &cells.trees, &cells.stages, &cells.wet, &cells.clouds, &cells.raining].into_iter().for_each(|words| written.words(words));
        if !cells.heights.is_empty() {
            cells.water.depths.iter().chain([&cells.water.deep]).for_each(|words| written.words(words));
        }
        written.words(&cells.sheep.iter().map(|&(x, y)| u64::from(x) << 16 | u64::from(y)).collect::<Vec<u64>>());
    }
    written.0
}

/// What [`frame_written`] wrote: `waters` the water of each
/// superchunk whose heights came, kept for the frames after.
pub(crate) fn frame_read(words: &[u64], waters: &mut HashMap<(u32, u32), Arc<Water>>) -> Option<Frame> {
    let mut read = Read64::of(words);
    let [world, seed, tick, ticks_a_second, sheep, grass, trees, sync_seconds, sync_share, detail, more] = std::array::from_fn(|_| read.number());
    let numbers: Vec<(String, u64)> = (0..read.number()?).map(|_| Some((read.text()?, read.number()?))).collect::<Option<_>>()?;
    let generation = Generation::of_numbers(&numbers);
    let side = read.maybe(|read| read.number().map(|side| side as u32))?;
    let (viewport, near) = (read.maybe(viewport_read)?, read.maybe(near_read)?);
    let (named, said) = (read.maybe(Read64::text)?, read.maybe(Read64::text)?);
    let hot: Vec<(u32, u32)> = (0..read.number()?).map(|_| read.pair()).collect::<Option<_>>()?;
    let mut cells = Vec::new();
    for _ in 0..read.number()? {
        let (at, top_left) = (read.pair()?, read.pair()?);
        let [grass, heights, trees, stages, wet, clouds, raining] = [read.words()?, read.words()?, read.words()?, read.words()?, read.words()?, read.words()?, read.words()?];
        if !heights.is_empty() {
            let water = Water { depths: [read.words()?, read.words()?, read.words()?, read.words()?], deep: read.words()? };
            waters.insert(at, Arc::new(water));
        }
        let sheep = read.words()?.into_iter().map(|word| ((word >> 16) as u16, word as u16)).collect();
        let water = Arc::clone(waters.entry(at).or_default());
        cells.push(Cells { at, grass, top_left, heights, trees, stages, wet, clouds, raining, water, sheep });
    }
    Some(Frame { world: world?, seed: seed?, levels: Levels::of(&generation), generation, side, tick: tick?, ticks_a_second: f64::from_bits(ticks_a_second?), sheep: sheep? as usize, grass: grass?, trees: trees?, sync_seconds: f64::from_bits(sync_seconds?), sync_share: f64::from_bits(sync_share?), viewport, hot, detail: detail? as u32, near, named, said, more: more? == 1, cells })
}
