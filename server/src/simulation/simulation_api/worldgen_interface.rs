//! A world generation's interface to the simulation and the server
//! ([`WorldgenCode`]): set up once as its world is set up and as it runs
//! -- its own config read from its mod's folder of the world's mod
//! config, its defaults where there is none -- what it gives back then
//! generating each superchunk never made, whole, on its warming's job.
//! It is the library
//! of the type hooked into generating the world. Its config is its own:
//! the server hands over the folder and reads nothing in it.

use crate::simulation::halos::GeneratingSuperchunk;
use std::path::Path;
use std::sync::Arc;

/// Generates a superchunk never made of the world of a seed, whole --
/// its cells and what lives on it -- on its warming's job, off the
/// tick: the same whenever it is made.
pub type Generate = dyn Fn(u64, &mut GeneratingSuperchunk) + Send + Sync;

/// Sets a world generation up for one world, given its mod's folder of
/// the world's mod config: or why its config cannot be read.
pub type SetUp = dyn Fn(&Path) -> Result<SetUpWorldgen, String> + Send + Sync;

/// A world generation set up for one world: its config read, what it
/// generates fixed for as long as the world runs.
#[derive(Clone)]
pub struct SetUpWorldgen {
    /// Generates a superchunk never made, whole.
    pub generate: Arc<Generate>,
}

/// What generates a world, as the library of the type hooked into
/// generating it: set up for each world, from its config.
#[derive(Clone)]
pub struct WorldgenCode {
    /// Sets it up for one world.
    pub set_up: Arc<SetUp>,
}

impl std::fmt::Debug for WorldgenCode {
    /// Nothing of it: what generates the world, only its code knows.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.debug_struct("WorldgenCode").finish_non_exhaustive()
    }
}
