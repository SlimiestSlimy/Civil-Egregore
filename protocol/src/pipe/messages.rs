//! What the client sends the server ([`ClientMessage`]) and the server
//! the client ([`ServerMessage`]), each on the wire as a word naming
//! which it is, then what it carries.

use super::frame::{FrameAsk, ViewportFrame};
use super::terrain::{Height, HeightsAsk, MapAnswer, MapAsk};
use utilities::tuning::Tuning;
use utilities::wire::{WireReader, Wired};

/// Ticks a second a world is held to unless told otherwise: the
/// game's target, a second of the simulation in a second.
pub const TARGET_PACE: u32 = simulation::simulation_time::TICKS_A_SIMULATION_SECOND as u32;

/// What a client asks of the server.
#[derive(Clone, Debug, Default)]
pub enum ClientMessage {
    /// Some of the viewport's superchunks: answered with a frame --
    /// unless no world runs, when nothing is.
    AskFrame(FrameAsk),
    /// Stop ticking, or go on.
    SetPaused(bool),
    /// Tick so many times a second, or flat out.
    SetPace(Option<u32>),
    /// Make a world from the seed, or one drawn, as the sliders' numbers
    /// say, and run it in place of any run.
    MakeWorld(Option<u64>, Tuning),
    /// Run the world of this name in the worlds' folder in place of any run.
    OpenWorld(String),
    /// Save the world run under this name in the worlds' folder.
    Save(String),
    /// Make the world run again from its start, generated as the
    /// sliders' numbers say.
    Reset(Tuning),
    /// The heights of a rectangle of cells: answered with them.
    AskHeights(HeightsAsk),
    /// A map: answered with it.
    AskMap(MapAsk),
    /// The names of the worlds in the worlds' folder: answered with them.
    #[default]
    ListWorlds,
}

/// What the server tells a client.
#[derive(Clone)]
pub enum ServerMessage {
    /// Some of the viewport's superchunks, as a tick left them.
    Frame(Box<ViewportFrame>),
    /// The heights asked for, row by row: none if another world runs
    /// than the one asked of.
    Heights(Option<Vec<Height>>),
    /// The map asked for: none if another world runs than the one asked of.
    Map(Option<MapAnswer>),
    /// The names of the worlds in the worlds' folder.
    Worlds(Vec<String>),
}

impl Default for ServerMessage {
    /// No worlds: what a message taken off the wire is filled in.
    fn default() -> Self {
        Self::Worlds(Vec::new())
    }
}

impl Wired for ClientMessage {
    fn put(&mut self, words: &mut Vec<u64>) {
        match self {
            Self::AskFrame(frame_ask) => {
                words.push(0);
                frame_ask.put(words);
            }
            Self::SetPaused(paused) => {
                words.push(1);
                paused.put(words);
            }
            Self::SetPace(pace) => {
                words.push(2);
                pace.put(words);
            }
            Self::MakeWorld(seed, tuning) => {
                words.push(3);
                seed.put(words);
                tuning.put(words);
            }
            Self::OpenWorld(world_name) => {
                words.push(4);
                world_name.put(words);
            }
            Self::Save(world_name) => {
                words.push(5);
                world_name.put(words);
            }
            Self::Reset(tuning) => {
                words.push(6);
                tuning.put(words);
            }
            Self::AskHeights(heights_ask) => {
                words.push(7);
                heights_ask.put(words);
            }
            Self::AskMap(map_ask) => {
                words.push(8);
                map_ask.put(words);
            }
            Self::ListWorlds => words.push(9),
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => Self::AskFrame(taken(Default::default(), wire)?),
            1 => Self::SetPaused(taken(Default::default(), wire)?),
            2 => Self::SetPace(taken(Default::default(), wire)?),
            3 => Self::MakeWorld(taken(None, wire)?, taken([0.0; _], wire)?),
            4 => Self::OpenWorld(taken(Default::default(), wire)?),
            5 => Self::Save(taken(Default::default(), wire)?),
            6 => Self::Reset(taken([0.0; _], wire)?),
            7 => Self::AskHeights(taken(Default::default(), wire)?),
            8 => Self::AskMap(taken(Default::default(), wire)?),
            9 => Self::ListWorlds,
            _ => return None,
        };
        Some(())
    }
}

impl Wired for ServerMessage {
    fn put(&mut self, words: &mut Vec<u64>) {
        match self {
            Self::Frame(frame) => {
                words.push(0);
                frame.put(words);
            }
            Self::Heights(heights) => {
                words.push(1);
                heights.put(words);
            }
            Self::Map(map_answer) => {
                words.push(2);
                map_answer.put(words);
            }
            Self::Worlds(world_names) => {
                words.push(3);
                world_names.put(words);
            }
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => Self::Frame(Box::new(taken(Default::default(), wire)?)),
            1 => Self::Heights(taken(Default::default(), wire)?),
            2 => Self::Map(taken(Default::default(), wire)?),
            3 => Self::Worlds(taken(Default::default(), wire)?),
            _ => return None,
        };
        Some(())
    }
}

/// The next value on `wire`, taken in place of `value`.
fn taken<Value: Wired>(mut value: Value, wire: &mut WireReader) -> Option<Value> {
    value.take(wire)?;
    Some(value)
}
