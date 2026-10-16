//! The wire between a ticker and the window showing its worlds: what the
//! window asks and the frames answered, as words of 64 bits over a
//! local socket -- so a ticker needs nothing of a window but this, and a
//! window nothing of a ticker's process (`docs/server.md`, "A world
//! shown").

use super::frame::{FrameAsk, Cells, ViewportFrame, NearRectangle, Viewport, Water};
use super::terrain::DrawnHeightRange;
use super::HostRequest;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::sync::Arc;
use worldgen::WorldGeneration;

/// The most words a message is read as: more is no message of ours.
const MOST_WORDS: u64 = 1 << 28;

/// Sends `words` as one message: how many, then each, low byte first.
pub(crate) fn send_message(stream_to: &mut impl Write, words: &[u64]) -> io::Result<()> {
    let mut message_bytes = Vec::with_capacity((words.len() + 1) * 8);
    message_bytes.extend_from_slice(&(words.len() as u64).to_le_bytes());
    words.iter().for_each(|word| message_bytes.extend_from_slice(&word.to_le_bytes()));
    stream_to.write_all(&message_bytes)?;
    stream_to.flush()
}

/// Reads one message [`send_message`] sent: its words.
pub(crate) fn receive_message(stream_from: &mut impl Read) -> io::Result<Vec<u64>> {
    let mut word = [0u8; 8];
    stream_from.read_exact(&mut word)?;
    let count = u64::from_le_bytes(word);
    if count > MOST_WORDS {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "a message longer than any sent"));
    }
    let mut message_bytes = vec![0u8; count as usize * 8];
    stream_from.read_exact(&mut message_bytes)?;
    Ok(message_bytes.as_chunks::<8>().0.iter().map(|word| u64::from_le_bytes(*word)).collect())
}

/// A message being written, word after word.
#[derive(Default)]
pub(crate) struct MessageWritten(pub(crate) Vec<u64>);

impl MessageWritten {
    /// One number.
    fn write_number(&mut self, number_written: u64) {
        self.0.push(number_written);
    }

    /// A pair of them, in one word.
    fn write_pair(&mut self, (high_half, low_half): (u32, u32)) {
        self.0.push(u64::from(high_half) << 32 | u64::from(low_half));
    }

    /// Words, how many before them.
    fn words(&mut self, words: &[u64]) {
        self.0.push(words.len() as u64);
        self.0.extend_from_slice(words);
    }

    /// A text, its bytes eight a word.
    fn write_text(&mut self, text_written: &str) {
        self.0.push(text_written.len() as u64);
        self.0.extend(text_written.as_bytes().chunks(8).map(|eight_bytes| eight_bytes.iter().rev().fold(0, |word, &text_byte| word << 8 | u64::from(text_byte))));
    }

    /// Something there may be none of: whether there is, then it.
    fn write_maybe<T>(&mut self, maybe_value: Option<T>, mut write_value: impl FnMut(&mut Self, T)) {
        self.0.push(u64::from(maybe_value.is_some()));
        if let Some(present_value) = maybe_value {
            write_value(self, present_value);
        }
    }
}

/// A message being read, word after word: `None` from any read past
/// its end, so a message cut short is no message.
pub(crate) struct MessageRead<'a> {
    /// Its words.
    words: &'a [u64],
    /// The next to read.
    next_word_index: usize,
}

impl<'a> MessageRead<'a> {
    /// `words`, from the first.
    pub(crate) fn over_words(words: &'a [u64]) -> Self {
        Self { words, next_word_index: 0 }
    }

    /// One number.
    fn read_number(&mut self) -> Option<u64> {
        self.next_word_index += 1;
        self.words.get(self.next_word_index - 1).copied()
    }

    /// A pair of them.
    fn read_pair(&mut self) -> Option<(u32, u32)> {
        self.read_number().map(|word| ((word >> 32) as u32, word as u32))
    }

    /// Words, how many before them.
    fn words(&mut self) -> Option<Vec<u64>> {
        let count = self.read_number()? as usize;
        let words = self.words.get(self.next_word_index..self.next_word_index.checked_add(count)?)?;
        self.next_word_index += count;
        Some(words.to_vec())
    }

    /// A text.
    fn read_text(&mut self) -> Option<String> {
        let byte_count = self.read_number()? as usize;
        let words = self.words.get(self.next_word_index..self.next_word_index.checked_add(byte_count.div_ceil(8))?)?;
        self.next_word_index += words.len();
        String::from_utf8(words.iter().flat_map(|word| word.to_le_bytes()).take(byte_count).collect()).ok()
    }

    /// Something there may be none of.
    fn read_maybe<T>(&mut self, read_value: impl FnOnce(&mut Self) -> Option<T>) -> Option<Option<T>> {
        match self.read_number()? {
            0 => Some(None),
            _ => read_value(self).map(Some),
        }
    }
}

/// A viewport's two corners.
fn viewport_written(message_written: &mut MessageWritten, viewport: Viewport) {
    message_written.write_pair(viewport.top_left_superchunk_cartesian);
    message_written.write_pair(viewport.bottom_right_superchunk_cartesian);
}

/// A viewport read.
fn viewport_read(message_read: &mut MessageRead) -> Option<Viewport> {
    Some(Viewport { top_left_superchunk_cartesian: message_read.read_pair()?, bottom_right_superchunk_cartesian: message_read.read_pair()? })
}

/// The cells seen from near, written.
fn near_written(message_written: &mut MessageWritten, seen_near: NearRectangle) {
    message_written.write_pair(seen_near.first_cell_cartesian);
    message_written.write_pair(seen_near.size_in_cells);
    message_written.write_number(u64::from(seen_near.pixels_a_cell));
}

/// The cells seen from near, read.
fn near_read(message_read: &mut MessageRead) -> Option<NearRectangle> {
    Some(NearRectangle { first_cell_cartesian: message_read.read_pair()?, size_in_cells: message_read.read_pair()?, pixels_a_cell: message_read.read_number()? as u32 })
}

/// What a window asks of a ticker, as a message: none of what only a
/// host that owns its world can do -- make one, open one, save one.
pub(crate) fn request_written(host_request: &HostRequest) -> Option<Vec<u64>> {
    let mut message_written = MessageWritten::default();
    match host_request {
        HostRequest::AskFrame(frame_ask) => {
            message_written.write_number(0);
            message_written.write_maybe(frame_ask.viewport, viewport_written);
            message_written.write_pair((frame_ask.pixel_side_log2, frame_ask.superchunks_skipped));
            message_written.write_number(u64::from(frame_ask.most_superchunks));
            message_written.write_maybe(frame_ask.seen_near, near_written);
        }
        HostRequest::SetPaused(pause_asked) => message_written.write_pair((1, u32::from(*pause_asked))),
        HostRequest::SetPace(pace_asked) => {
            message_written.write_number(2);
            message_written.write_maybe(*pace_asked, |pace_written, ticks_a_second| pace_written.write_number(u64::from(ticks_a_second)));
        }
        HostRequest::MakeWorld(_) | HostRequest::OpenWorld(_) | HostRequest::Save(_) | HostRequest::Reset(_) => return None,
    }
    Some(message_written.0)
}

/// What [`request_written`] wrote.
pub(crate) fn request_read(words: &[u64]) -> Option<HostRequest> {
    let mut message_read = MessageRead::over_words(words);
    Some(match message_read.read_number()? {
        0 => {
            let viewport = message_read.read_maybe(viewport_read)?;
            let ((pixel_side_log2, superchunks_skipped), most_superchunks) = (message_read.read_pair()?, message_read.read_number()? as u32);
            HostRequest::AskFrame(FrameAsk { viewport, pixel_side_log2, superchunks_skipped, most_superchunks, seen_near: message_read.read_maybe(near_read)? })
        }
        2 => HostRequest::SetPace(message_read.read_maybe(|pace_read| pace_read.read_number().map(|ticks_a_second| ticks_a_second as u32))?),
        pause_word => HostRequest::SetPaused(pause_word & 1 == 1),
    })
}

/// A frame as a message. A superchunk's water goes with its heights,
/// the once they are sent: the window keeps it from then.
pub(crate) fn frame_written(viewport_frame: &ViewportFrame) -> Vec<u64> {
    let mut message_written = MessageWritten::default();
    [viewport_frame.world, viewport_frame.seed, viewport_frame.tick, viewport_frame.ticks_a_second.to_bits(), viewport_frame.sheep_count as u64, viewport_frame.grass_cells, viewport_frame.trees, viewport_frame.sync_seconds.to_bits(), viewport_frame.sync_share.to_bits(), u64::from(viewport_frame.pixel_side_log2), u64::from(viewport_frame.more_to_come)].into_iter().for_each(|frame_number| message_written.write_number(frame_number));
    let generation_numbers = viewport_frame.world_generation.numbers();
    message_written.write_number(generation_numbers.len() as u64);
    for (number_name, number_value) in &generation_numbers {
        message_written.write_text(number_name);
        message_written.write_number(*number_value);
    }
    message_written.write_maybe(viewport_frame.side_superchunks, |side_written, side_number| side_written.write_number(u64::from(side_number)));
    message_written.write_maybe(viewport_frame.viewport, viewport_written);
    message_written.write_maybe(viewport_frame.seen_near, near_written);
    message_written.write_maybe(viewport_frame.world_name.as_deref(), MessageWritten::write_text);
    message_written.write_maybe(viewport_frame.last_outcome.as_deref(), MessageWritten::write_text);
    message_written.write_number(viewport_frame.hot.len() as u64);
    viewport_frame.hot.iter().for_each(|&superchunk_cartesian| message_written.write_pair(superchunk_cartesian));
    message_written.write_number(viewport_frame.cells.len() as u64);
    for cells in &viewport_frame.cells {
        message_written.write_pair(cells.superchunk_cartesian);
        message_written.write_pair(cells.top_left);
        [&cells.grass_bitmaps, &cells.heights, &cells.trees, &cells.stages, &cells.wet_bitmaps, &cells.clouds, &cells.raining_tiles, &cells.climates].into_iter().for_each(|words| message_written.words(words));
        if !cells.heights.is_empty() {
            cells.water.depth_low_bits.iter().chain([&cells.water.deep_cells]).for_each(|words| message_written.words(words));
        }
        message_written.words(&cells.sheep_cells_cartesian.iter().map(|&(x, y)| u64::from(x) << 16 | u64::from(y)).collect::<Vec<u64>>());
    }
    message_written.0
}

/// What [`frame_written`] wrote: `waters` the water of each
/// superchunk whose heights came, kept for the frames after.
pub(crate) fn frame_read(words: &[u64], waters: &mut HashMap<(u32, u32), Arc<Water>>) -> Option<ViewportFrame> {
    let mut message_read = MessageRead::over_words(words);
    let [world, seed, tick, ticks_a_second, sheep_count, grass_cells, trees, sync_seconds, sync_share, pixel_side_log2, more_to_come] = std::array::from_fn(|_| message_read.read_number());
    let generation_numbers: Vec<(String, u64)> = (0..message_read.read_number()?).map(|_| Some((message_read.read_text()?, message_read.read_number()?))).collect::<Option<_>>()?;
    let world_generation = WorldGeneration::of_numbers(&generation_numbers);
    let side_superchunks = message_read.read_maybe(|side_read| side_read.read_number().map(|side_number| side_number as u32))?;
    let (viewport, seen_near) = (message_read.read_maybe(viewport_read)?, message_read.read_maybe(near_read)?);
    let (world_name, last_outcome) = (message_read.read_maybe(MessageRead::read_text)?, message_read.read_maybe(MessageRead::read_text)?);
    let hot: Vec<(u32, u32)> = (0..message_read.read_number()?).map(|_| message_read.read_pair()).collect::<Option<_>>()?;
    let mut cells = Vec::new();
    for _ in 0..message_read.read_number()? {
        let (superchunk_cartesian, top_left) = (message_read.read_pair()?, message_read.read_pair()?);
        let [grass_bitmaps, heights, trees, stages, wet_bitmaps, clouds, raining_tiles, climates] = [message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?];
        if !heights.is_empty() {
            let water = Water { depth_low_bits: [message_read.words()?, message_read.words()?, message_read.words()?, message_read.words()?], deep_cells: message_read.words()? };
            waters.insert(superchunk_cartesian, Arc::new(water));
        }
        let sheep_cells_cartesian = message_read.words()?.into_iter().map(|word| ((word >> 16) as u16, word as u16)).collect();
        let water = Arc::clone(waters.entry(superchunk_cartesian).or_default());
        cells.push(Cells { superchunk_cartesian, grass_bitmaps, top_left, heights, trees, stages, wet_bitmaps, clouds, raining_tiles, climates, water, sheep_cells_cartesian });
    }
    Some(ViewportFrame { world: world?, seed: seed?, drawn_heights: DrawnHeightRange::from_generation(&world_generation), world_generation, side_superchunks, tick: tick?, ticks_a_second: f64::from_bits(ticks_a_second?), sheep_count: sheep_count? as usize, grass_cells: grass_cells?, trees: trees?, sync_seconds: f64::from_bits(sync_seconds?), sync_share: f64::from_bits(sync_share?), viewport, hot, pixel_side_log2: pixel_side_log2? as u32, seen_near, world_name, last_outcome, more_to_come: more_to_come? == 1, cells })
}
