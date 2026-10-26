//! Battery of the computer itself, e.g. a laptop.

use crate::battery::{Battery, Kind, Link, Pack, Power};
use crate::platform;
use std::time::Duration;

/// Estimates beyond this come from a rate too small to be meaningful.
const MAX_ESTIMATE: Duration = Duration::from_secs(24 * 60 * 60);

/// What the system reports about the computer's battery as a whole.
pub struct Status {
    pub percent: u8,
    pub power: Power,
    /// The system's own estimate of the time left while discharging.
    pub remaining: Option<Duration>,
}

pub fn battery() -> Option<Battery> {
    let status = platform::host::status()?;
    let name = platform::host::model()
        .or_else(platform::host::hostname)
        .unwrap_or_else(|| "This computer".into());
    let pack = pack();
    let remaining = match status.power {
        // The system smooths its estimate over a while, so it is steadier than one computed from the current rate.
        Power::Discharging if status.remaining.is_some() => status.remaining,
        Power::Discharging | Power::Charging => pack.and_then(|pack| time_at_rate(&pack)),
        _ => None,
    };
    Some(Battery {
        name,
        kind: Kind::Computer,
        manufacturer: platform::host::manufacturer().unwrap_or_default(),
        connection: "Built-in battery".into(),
        link: Link::Builtin,
        percent: status.percent,
        power: status.power,
        rechargeable: true,
        remaining,
        pack,
        reported: None,
        connected: true,
    })
}

/// Time until the battery is full while charging, or empty while discharging, at its current rate.
fn time_at_rate(pack: &Pack) -> Option<Duration> {
    let rate = pack.rate.filter(|&rate| rate != 0)?;
    let energy = if rate > 0 {
        pack.full.saturating_sub(pack.now)
    } else {
        pack.now
    };
    let hours = energy as f32 / rate.unsigned_abs() as f32;
    Some(Duration::from_secs_f32(hours * 3600.0)).filter(|&time| time < MAX_ESTIMATE)
}

/// Sums up the capacity and wear of every battery.
fn pack() -> Option<Pack> {
    platform::host::packs().reduce(|a, b| Pack {
        now: a.now + b.now,
        full: a.full + b.full,
        design: a.design + b.design,
        rate: a.rate.zip(b.rate).map(|(a, b)| a + b),
        cycles: a.cycles.max(b.cycles),
    })
}
