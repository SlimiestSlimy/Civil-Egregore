//! The parameters a command line gives the server: the shell's words as
//! they came, and the same read by the flags into a tree
//! (`docs/reference.md`, "server_parameters.rs").

use crate::flags::{Flag, FlagError, FlagParser};
use std::collections::BTreeMap;

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
#[derive(Default)]
pub struct CleanServerParameters {
    /// Every parameter, by its name.
    parameter_values: BTreeMap<&'static str, ParameterValue>,
}

/// A parameter's value: typed as its token was, or a flag's own
/// parameters, nested.
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
    pub fn insert(&mut self, parameter_name: &'static str, parameter_value: ParameterValue) {
        self.parameter_values.insert(parameter_name, parameter_value);
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
