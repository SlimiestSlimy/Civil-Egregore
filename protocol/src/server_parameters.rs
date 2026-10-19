//! The parameters a command line gives the server: the shell's words as
//! they came, and the same read by the flags into a tree
//! (`docs/reference.md`, "server_parameters.rs") -- the tree being too
//! what a world is started by, whoever starts it.

use crate::flags::{Flag, FlagError, FlagParser};
use std::collections::BTreeMap;
use utilities::wire::{WireReader, Wired};

// The parameters a world is started by, whoever starts it: a command
// line, or a window through the host.

/// The world's seed: none given, one drawn that its generation finds
/// fit to start.
pub const SEED: &str = "seed";
/// Superchunks along a world's side: none, as far as it goes.
pub const SIDE: &str = "side";
/// A world hot throughout, not about its hot entities' halos.
pub const FORCED_HOT: &str = "forced_hot";
/// Threads a world ticks on: none, every one the machine has.
pub const THREADS: &str = "threads";
/// A world's folder: a name in the worlds' folder, or a path. The world
/// saved there is opened -- every parameter of a new one nothing to it
/// -- or, none saved there, a new one is made and saved there.
pub const WORLD: &str = "world";
/// Bytes of its cold superchunks' images an opened world keeps in
/// memory: none given, the server's own.
pub const COLD_POOL_BYTES: &str = "cold_pool_bytes";
/// A new world's camera loads superchunks: the viewport's hot as well.
pub const CAMERA_LOADS: &str = "camera_loads";
/// A new world's worldgen parameters, each by its name: its
/// generation's own, handed it untouched.
pub const WORLDGEN: &str = "worldgen";

/// The command line as the shell gave it: words, nothing read yet.
pub struct ServerParameters {
    /// The words after the program's name.
    pub shell_words: Vec<String>,
}

impl ServerParameters {
    /// The words read by `flags` into what the server takes.
    pub fn cleaned(&self, flags: Vec<Flag>) -> Result<CleanServerParameters, FlagError> {
        let mut clean_parameters = CleanServerParameters::default();
        FlagParser::new(&self.shell_words, flags).parse(&mut clean_parameters)?;
        Ok(clean_parameters)
    }
}

/// The command line read: every parameter by name, a flag's bracketed
/// parameters nested under it. By name and not by field, so what the
/// type registry adds once it is dynamic can bring parameters of its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CleanServerParameters {
    /// Every parameter, by its name.
    parameter_values: BTreeMap<String, ParameterValue>,
}

/// A parameter's value: typed as its token was, or a flag's own
/// parameters, nested.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    /// A number in hexadecimal: a seed.
    Hex(u64),
    /// A whole number.
    Integer(i64),
    /// A number with a fraction.
    Float(f64),
    /// A word.
    Text(String),
    /// A flag's own parameters.
    Nested(CleanServerParameters),
}

impl CleanServerParameters {
    /// Puts `parameter_value` under `parameter_name`, over any before.
    pub fn insert(&mut self, parameter_name: impl Into<String>, parameter_value: ParameterValue) {
        self.parameter_values.insert(parameter_name.into(), parameter_value);
    }

    /// With `parameter_value` under `parameter_name`, over any before.
    pub fn with(mut self, parameter_name: impl Into<String>, parameter_value: ParameterValue) -> Self {
        self.insert(parameter_name, parameter_value);
        self
    }

    /// Every parameter, by its name: what one whose names are not known
    /// beforehand -- a type's own -- is read by.
    pub fn each(&self) -> impl Iterator<Item = (&str, &ParameterValue)> {
        self.parameter_values.iter().map(|(parameter_name, parameter_value)| (parameter_name.as_str(), parameter_value))
    }

    /// The parameter `parameter_name`, if given in hexadecimal.
    pub fn hex(&self, parameter_name: &str) -> Option<u64> {
        match self.parameter_values.get(parameter_name)? {
            ParameterValue::Hex(hex_number) => Some(*hex_number),
            _ => None,
        }
    }

    /// The parameter `parameter_name`, if given as a whole number.
    pub fn integer(&self, parameter_name: &str) -> Option<i64> {
        match self.parameter_values.get(parameter_name)? {
            ParameterValue::Integer(integer) => Some(*integer),
            _ => None,
        }
    }

    /// The parameter `parameter_name`, if given as a number.
    pub fn float(&self, parameter_name: &str) -> Option<f64> {
        match self.parameter_values.get(parameter_name)? {
            ParameterValue::Float(float) => Some(*float),
            _ => None,
        }
    }

    /// The parameter `parameter_name`, if given as a word.
    pub fn text(&self, parameter_name: &str) -> Option<&str> {
        match self.parameter_values.get(parameter_name)? {
            ParameterValue::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The flag `parameter_name`'s own parameters, if it was given.
    pub fn nested(&self, parameter_name: &str) -> Option<&CleanServerParameters> {
        match self.parameter_values.get(parameter_name)? {
            ParameterValue::Nested(nested_parameters) => Some(nested_parameters),
            _ => None,
        }
    }
}

impl Default for ParameterValue {
    /// A flag given with nothing of its own: what a value taken off the
    /// wire is filled in.
    fn default() -> Self {
        Self::Nested(CleanServerParameters::default())
    }
}

impl Wired for ParameterValue {
    fn put(&mut self, words: &mut Vec<u64>) {
        match self {
            Self::Hex(hex_number) => {
                words.push(0);
                hex_number.put(words);
            }
            Self::Integer(integer) => {
                words.push(1);
                integer.put(words);
            }
            Self::Float(float) => {
                words.push(2);
                float.put(words);
            }
            Self::Text(text) => {
                words.push(3);
                text.put(words);
            }
            Self::Nested(nested_parameters) => {
                words.push(4);
                nested_parameters.put(words);
            }
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => Self::Hex(taken(0, wire)?),
            1 => Self::Integer(taken(0, wire)?),
            2 => Self::Float(taken(0.0, wire)?),
            3 => Self::Text(taken(String::new(), wire)?),
            4 => Self::Nested(taken(CleanServerParameters::default(), wire)?),
            _ => return None,
        };
        Some(())
    }
}

impl Wired for CleanServerParameters {
    /// Every parameter, by its name: what a client starts a world by, as
    /// a command line would.
    fn put(&mut self, words: &mut Vec<u64>) {
        let mut each: Vec<(String, ParameterValue)> = self.parameter_values.iter().map(|(parameter_name, parameter_value)| (parameter_name.clone(), parameter_value.clone())).collect();
        each.put(words);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        self.parameter_values = taken(Vec::<(String, ParameterValue)>::new(), wire)?.into_iter().collect();
        Some(())
    }
}

/// The next value on `wire`, taken in place of `value`.
fn taken<Value: Wired>(mut value: Value, wire: &mut WireReader) -> Option<Value> {
    value.take(wire)?;
    Some(value)
}
