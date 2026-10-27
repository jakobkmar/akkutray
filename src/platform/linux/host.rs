//! The computer's battery and model from sysfs.

use crate::battery::{Pack, Power};
use crate::host::Status;
use std::fs;
use std::path::{Path, PathBuf};

const POWER_SUPPLY: &str = "/sys/class/power_supply";
/// The SMBIOS system information.
const DMI: &str = "/sys/class/dmi/id";

pub fn status() -> Option<Status> {
    let batteries = batteries();
    let levels: Vec<u64> = batteries
        .iter()
        .filter_map(|battery| number(battery, "capacity"))
        .collect();
    if levels.is_empty() {
        return None;
    }
    // ponytail: averages the levels, which is only exact for batteries of the same size
    let percent = (levels.iter().sum::<u64>() / levels.len() as u64).min(100);
    let states: Vec<String> = batteries.iter().filter_map(|battery| text(battery, "status")).collect();
    // One charging battery makes the computer charge, while another may wait, e.g. in a laptop with two.
    let power = [
        ("Charging", Power::Charging),
        ("Discharging", Power::Discharging),
        // The driver says so when a charge limit holds the battery below full.
        ("Not charging", Power::PluggedIn),
        ("Full", Power::Full),
    ]
    .into_iter()
    .find(|(state, _)| states.iter().any(|s| s == state))
    .map_or(Power::Unknown, |(_, power)| power);
    Some(Status {
        percent: percent as u8,
        power,
        // Linux keeps no estimate of its own.
        remaining: None,
    })
}

pub fn model() -> Option<String> {
    text(Path::new(DMI), "product_name")
}

pub fn manufacturer() -> Option<String> {
    text(Path::new(DMI), "sys_vendor")
}

pub fn hostname() -> Option<String> {
    text(Path::new("/proc/sys/kernel"), "hostname")
}

/// Reads the capacity and wear of every battery from its driver.
pub fn packs() -> impl Iterator<Item = Pack> {
    batteries().into_iter().filter_map(|battery| pack(&battery))
}

fn pack(battery: &Path) -> Option<Pack> {
    let get = |name: &str| number(battery, name);
    // Drivers report either energy in microwatt-hours, or charge in microampere-hours, which the design voltage in
    // microvolts turns into energy. Power comes in microwatts, or as current at the present voltage, without a sign.
    let energy = |name: &str| {
        get(&format!("energy_{name}"))
            .or_else(|| Some(get(&format!("charge_{name}"))? * get("voltage_min_design")? / 1_000_000))
    };
    let milli = |micro: u64| u32::try_from(micro / 1000).ok();
    let design = milli(energy("full_design")?).filter(|&design| design != 0)?;
    let rate = get("power_now")
        .or_else(|| Some(get("current_now")? * get("voltage_now")? / 1_000_000))
        .and_then(milli)
        .map(|rate| rate as i32);
    let discharging = text(battery, "status").is_some_and(|status| status == "Discharging");
    Some(Pack {
        now: milli(energy("now")?)?,
        full: milli(energy("full")?)?,
        design,
        rate: rate.map(|rate| if discharging { -rate } else { rate }),
        // Batteries that don't count their cycles report 0.
        cycles: get("cycle_count")
            .filter(|&cycles| cycles != 0)
            .map(|cycles| cycles as u32),
    })
}

/// The batteries of the computer, leaving out those of devices such as mice, which Linux lists as well.
fn batteries() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            text(path, "type").is_some_and(|kind| kind == "Battery")
                && text(path, "scope").is_none_or(|scope| scope == "System")
                && number(path, "present") != Some(0)
        })
        .collect()
}

fn text(dir: &Path, name: &str) -> Option<String> {
    let value = fs::read_to_string(dir.join(name)).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn number(dir: &Path, name: &str) -> Option<u64> {
    text(dir, name)?.parse().ok()
}
