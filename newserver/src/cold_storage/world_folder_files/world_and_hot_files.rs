//! A saved world's two text files: what the world is, and which of its
//! superchunks were hot, cooling and warming.

use protocol::type_registry::PlaneType;
use utilities::coordinates::SuperchunkIndex;
use utilities::csv;

/// The world file's columns.
const WORLD_COLUMNS: [&str; 2] = ["world", "is"];
/// The world file's format, and the number of the one written.
const FORMAT_ROW: (&str, &str) = ("format", "7");

/// The name of the row saying a world's camera forces superchunks hot.
const CAMERA_FORCES_SUPERCHUNKS_HOT: &str = "camera forces superchunks hot";

/// What a world is. Its name is its folder's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldInfo {
    /// Its seed: what everything generated and drawn comes from -- given
    /// as it was created, or drawn then.
    pub seed: u64,
    /// The tick it is at: the next to run.
    pub tick: u64,
    /// Its plane types: made hot on every chunk when it is loaded.
    pub plane_types: Vec<PlaneType>,
    /// Superchunks along a side of it, if it has a size: a square
    /// about its origin, nothing ever made outside it.
    pub side_in_superchunks: Option<u32>,
    /// Whether every superchunk of it is hot throughout, whatever its
    /// entities do: only one with a size can be.
    pub forced_hot: bool,
    /// Whether its camera forces superchunks hot: the viewport's hot as well.
    pub camera_forces_superchunks_hot: bool,
}

impl WorldInfo {
    /// As the world's file holds it: a row a thing the world is, its
    /// name and what it is -- its seed in hexadecimal. Its plane types
    /// are its registry's, not the file's.
    pub(crate) fn to_text(&self) -> String {
        let mut name_value_rows = vec![
            (FORMAT_ROW.0.to_string(), FORMAT_ROW.1.to_string()),
            ("seed".to_string(), utilities::seed::seed_as_hex(self.seed)),
            ("tick".to_string(), self.tick.to_string()),
        ];
        name_value_rows.extend(self.side_in_superchunks.map(|side_value| ("side".to_string(), side_value.to_string())));
        name_value_rows.extend(self.forced_hot.then(|| ("forced".to_string(), "1".to_string())));
        name_value_rows.extend(self.camera_forces_superchunks_hot.then(|| (CAMERA_FORCES_SUPERCHUNKS_HOT.to_string(), "1".to_string())));
        csv::row_text(&WORLD_COLUMNS) + &name_value_rows.iter().map(|(row_name, row_value)| csv::row_text(&[row_name, row_value])).collect::<String>()
    }

    /// From the world's file's text, or what is wrong with it: rows in
    /// any order, a missing one its default, all but the seed
    /// (`docs/chunk_storage.md`, "On disk").
    pub(crate) fn from_text(file_text: &str) -> Result<Self, String> {
        let mut csv_rows = csv::rows_with_header(file_text).into_iter();
        if csv_rows.next().is_none_or(|first_row| first_row != WORLD_COLUMNS) {
            return Err(format!("does not start with `{}`", WORLD_COLUMNS.join(",")));
        }
        let mut read_info = Self { seed: 0, tick: 0, plane_types: Vec::new(), side_in_superchunks: None, forced_hot: false, camera_forces_superchunks_hot: false };
        let (mut seed, mut names_seen) = (None, Vec::new());
        for csv_row in csv_rows {
            let [raw_name, value] = &csv_row[..] else {
                return Err(format!("`{}` is not a name and what it is", csv_row.join(",")));
            };
            let (row_name, value) = (raw_name.trim(), value.trim());
            if names_seen.iter().any(|seen_name| seen_name == row_name) {
                return Err(format!("`{row_name}` said twice"));
            }
            names_seen.push(row_name.to_string());
            let parse_number = || value.parse::<u64>().map_err(|_| format!("`{value}` is not a number"));
            match row_name {
                _ if row_name == FORMAT_ROW.0 && value != FORMAT_ROW.1 => return Err(format!("format {value}, not {}", FORMAT_ROW.1)),
                _ if row_name == FORMAT_ROW.0 => {}
                "seed" => seed = Some(utilities::seed::of_hex(value).ok_or_else(|| format!("`{value}` is not a seed"))?),
                "tick" => read_info.tick = parse_number()?,
                "side" => read_info.side_in_superchunks = Some(value.parse().map_err(|_| format!("`{value}` is not a side"))?),
                "forced" => read_info.forced_hot = parse_number()? != 0,
                CAMERA_FORCES_SUPERCHUNKS_HOT => read_info.camera_forces_superchunks_hot = parse_number()? != 0,
                // A name of another format: passed over.
                _ => {}
            }
        }
        if read_info.forced_hot && read_info.side_in_superchunks.is_none() {
            return Err("forced hot, with no side to be hot to".to_string());
        }
        read_info.seed = seed.ok_or("no seed")?;
        Ok(read_info)
    }
}

/// The hot file's columns.
const HOT_COLUMNS: [&str; 3] = ["superchunk", "is", "until"];
/// What a superchunk hot and not cooling is, in the hot file.
const HOT: &str = "hot";
/// What a hot superchunk cooling is, in the hot file.
const COOLING: &str = "cooling";
/// What a superchunk warming is, in the hot file.
const WARMING: &str = "warming";

/// Which superchunks a world had hot when saved: made hot again as it is
/// loaded, before it ticks. A row a superchunk: its index in
/// hexadecimal, then `hot`, or `cooling` and the tick it goes cold at
/// if hot and cooling, or `warming` and the tick it turns hot at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HotSuperchunks {
    /// The hot superchunks, sorted.
    pub hot: Vec<SuperchunkIndex>,
    /// The hot superchunks cooling, sorted, each with the tick it goes
    /// cold at.
    pub cooling: Vec<(SuperchunkIndex, u64)>,
    /// The superchunks warming, sorted, each with the tick it turns hot
    /// at.
    pub warming: Vec<(SuperchunkIndex, u64)>,
}

impl HotSuperchunks {
    /// As the hot file holds it.
    pub(crate) fn to_text(&self) -> String {
        let cooling_tick_of = |superchunk: &SuperchunkIndex| self.cooling.binary_search_by_key(superchunk, |&(cooling, _)| cooling).ok().map(|cooling_index| self.cooling[cooling_index].1);
        let hex_index = |superchunk: &SuperchunkIndex| format!("{:011x}", superchunk.0);
        let hot = self.hot.iter().map(|superchunk| match cooling_tick_of(superchunk) {
            Some(tick) => csv::row_text(&[hex_index(superchunk), COOLING.to_string(), tick.to_string()]),
            None => csv::row_text(&[hex_index(superchunk), HOT.to_string(), String::new()]),
        });
        let warming = self.warming.iter().map(|(superchunk, tick)| csv::row_text(&[hex_index(superchunk), WARMING.to_string(), tick.to_string()]));
        std::iter::once(csv::row_text(&HOT_COLUMNS)).chain(hot).chain(warming).collect()
    }

    /// From the hot file's text, or what is wrong with it.
    pub(crate) fn from_text(file_text: &str) -> Result<Self, String> {
        let mut csv_rows = csv::rows_with_header(file_text).into_iter();
        if csv_rows.next().is_none_or(|first_row| first_row != HOT_COLUMNS) {
            return Err(format!("does not start with `{}`", HOT_COLUMNS.join(",")));
        }
        let mut superchunks_read = Self::default();
        for csv_row in csv_rows {
            let wrong_row = || format!("`{}` is not a superchunk index, `{HOT}`, `{COOLING}` or `{WARMING}`, and a tick unless hot", csv_row.join(","));
            let [superchunk, state_name, until_tick] = &csv_row[..] else {
                return Err(wrong_row());
            };
            let superchunk = SuperchunkIndex(u64::from_str_radix(superchunk.trim(), 16).map_err(|_| wrong_row())?);
            let tick = || until_tick.trim().parse().map_err(|_| wrong_row());
            match state_name.trim() {
                HOT if until_tick.trim().is_empty() => superchunks_read.hot.push(superchunk),
                COOLING => {
                    superchunks_read.hot.push(superchunk);
                    superchunks_read.cooling.push((superchunk, tick()?));
                }
                WARMING => superchunks_read.warming.push((superchunk, tick()?)),
                _ => return Err(wrong_row()),
            }
        }
        superchunks_read.hot.sort_unstable();
        superchunks_read.cooling.sort_unstable();
        superchunks_read.warming.sort_unstable();
        Ok(superchunks_read)
    }
}
