//! What the client sends the server ([`ClientMessage`]) and the server
//! the client ([`ServerMessage`]), each on the wire as a word naming
//! which it is, then what it carries.

use super::frame::{FrameAsk, ViewportFrame};
use crate::server_parameters::CleanServerParameters;
use utilities::wire::{WireReader, Wired};

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
    /// Make a world as these parameters say -- the server's own, as a
    /// command line gives them -- and run it in place of any run.
    MakeWorld(CleanServerParameters),
    /// Run the world of this name in the worlds' folder in place of any run.
    OpenWorld(String),
    /// Save the world run in its folder.
    Save,
    /// The names of the worlds in the worlds' folder: answered with them.
    #[default]
    ListWorlds,
    /// A call of the function whose type's ID this is
    /// (`crate::type_registry::ClientCallType`), with these words for
    /// arguments: answered with its answer, or why it is refused. What
    /// the words mean, the function's own code says, the same on both
    /// ends -- both have the world's types.
    Call(u64, Vec<u64>),
}

/// What the server tells a client.
#[derive(Clone)]
pub enum ServerMessage {
    /// A world runs, the `n`-th the server has run: its types as its
    /// save holds them (`crate::type_registry::FrozenTypeRegistry::to_text`),
    /// sent once, before any frame of it -- what the client freezes its
    /// own registry of the game's types with, each of the world's ID
    /// (`crate::world_types::of_a_saved_world`).
    WorldTypes(u64, String),
    /// Some of the viewport's superchunks, as a tick left them.
    Frame(Box<ViewportFrame>),
    /// The names of the worlds in the worlds' folder.
    Worlds(Vec<String>),
    /// A call's answer: its words.
    Answered(Vec<u64>),
    /// Why a call was refused: no world runs, no function the client may
    /// call is of its ID, or the function would not take its arguments.
    CallRefused(String),
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
            Self::MakeWorld(parameters) => {
                words.push(3);
                parameters.put(words);
            }
            Self::OpenWorld(world_name) => {
                words.push(4);
                world_name.put(words);
            }
            Self::Save => words.push(5),
            Self::ListWorlds => words.push(6),
            Self::Call(call, arguments) => {
                words.push(7);
                call.put(words);
                arguments.put(words);
            }
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => Self::AskFrame(taken(Default::default(), wire)?),
            1 => Self::SetPaused(taken(Default::default(), wire)?),
            2 => Self::SetPace(taken(Default::default(), wire)?),
            3 => Self::MakeWorld(taken(Default::default(), wire)?),
            4 => Self::OpenWorld(taken(Default::default(), wire)?),
            5 => Self::Save,
            6 => Self::ListWorlds,
            7 => Self::Call(taken(0, wire)?, taken(Vec::new(), wire)?),
            _ => return None,
        };
        Some(())
    }
}

impl ServerMessage {
    /// A call's answer as it goes back: its words, or why it was refused.
    pub fn of_answer(answer: Result<Vec<u64>, String>) -> Self {
        match answer {
            Ok(words) => Self::Answered(words),
            Err(why_refused) => Self::CallRefused(why_refused),
        }
    }

    /// The call's answer this is, if it is one.
    pub fn into_answer(self) -> Option<Result<Vec<u64>, String>> {
        match self {
            Self::Answered(words) => Some(Ok(words)),
            Self::CallRefused(why_refused) => Some(Err(why_refused)),
            _ => None,
        }
    }
}

impl Wired for ServerMessage {
    fn put(&mut self, words: &mut Vec<u64>) {
        match self {
            Self::Frame(frame) => {
                words.push(0);
                frame.put(words);
            }
            Self::Worlds(world_names) => {
                words.push(1);
                world_names.put(words);
            }
            Self::WorldTypes(world, types_text) => {
                words.push(2);
                world.put(words);
                types_text.put(words);
            }
            Self::Answered(answer) => {
                words.push(3);
                answer.put(words);
            }
            Self::CallRefused(why_refused) => {
                words.push(4);
                why_refused.put(words);
            }
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => Self::Frame(Box::new(taken(Default::default(), wire)?)),
            1 => Self::Worlds(taken(Default::default(), wire)?),
            2 => Self::WorldTypes(taken(0, wire)?, taken(String::new(), wire)?),
            3 => Self::Answered(taken(Vec::new(), wire)?),
            4 => Self::CallRefused(taken(String::new(), wire)?),
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
