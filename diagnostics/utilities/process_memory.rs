//! The process's memory as the system counts it, now and at its peak,
//! and tracked over a run: from `/proc/self/status`, so only on Linux.

/// What the process holds in physical memory, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryReading {
    /// Held now (`VmRSS`).
    pub resident_bytes: u64,
    /// The most held since the process started (`VmHWM`).
    pub peak_bytes: u64,
}

/// The process's memory now, if the system says.
pub fn process_memory() -> Option<MemoryReading> {
    let status_text = std::fs::read_to_string("/proc/self/status").ok()?;
    let status_field_bytes = |field_name: &str| {
        let line = status_text.lines().find(|line| line.starts_with(field_name))?;
        let field_kibibytes: u64 = line[field_name.len()..].trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(field_kibibytes * 1024)
    };
    Some(MemoryReading { resident_bytes: status_field_bytes("VmRSS:")?, peak_bytes: status_field_bytes("VmHWM:")? })
}

/// The process's memory read again and again over a run: how many
/// readings, their sum, and the system's peak at the last one.
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryTrack {
    /// Readings taken.
    readings: u64,
    /// Their resident bytes, added up.
    resident_sum: u64,
    /// The peak at the last reading.
    last_peak_bytes: u64,
}

impl MemoryTrack {
    /// Reads the process's memory now, if the system says.
    pub fn take_reading(&mut self) {
        if let Some(memory_reading) = process_memory() {
            self.readings += 1;
            self.resident_sum += memory_reading.resident_bytes;
            self.last_peak_bytes = memory_reading.peak_bytes;
        }
    }

    /// The average resident bytes over the readings, if any were taken.
    pub fn average_resident_bytes(&self) -> Option<u64> {
        (self.readings > 0).then(|| self.resident_sum / self.readings)
    }

    /// The most the process has held, at the last reading, if any.
    pub fn peak_bytes(&self) -> Option<u64> {
        (self.readings > 0).then_some(self.last_peak_bytes)
    }
}

/// `byte_count` in mebibytes, to one decimal: how a report shows memory.
pub fn mebibytes_text(byte_count: u64) -> String {
    format!("{:.1} MiB", byte_count as f64 / (1u64 << 20) as f64)
}
