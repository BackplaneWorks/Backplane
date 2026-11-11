//! Boat's sizes, what each costs a month on the person's plan, and what the always-on hub needs of it (D8). The rate per hour of `default` comes
//! from the account (`GET /limits`, `overagePrices.vmHourDollars`); the other sizes are that rate times Boat's multiplier for them (docs.boat.dev
//! machines and the usage API's `billingMultiplier`). Boat counts plan hours the same way, so a small Boat uses half an hour of the plan an hour.
use serde_json::{json, Value};

/// $/h of `default` when the account says nothing (Boat's list price, Nov 2026).
pub const DEFAULT_RATE: f64 = 0.036;
/// Hours in a month, as Boat's price page counts one.
pub const MONTH_H: f64 = 730.0;

pub struct Size { pub id: &'static str, pub label: &'static str, pub vcpu: u32, pub ram_gb: f64, pub mult: f64 }
pub const SIZES: &[Size] = &[
    Size { id: "small", label: "Small", vcpu: 2, ram_gb: 4.0, mult: 0.5 },
    Size { id: "default", label: "Medium", vcpu: 4, ram_gb: 8.0, mult: 1.0 },
    Size { id: "large", label: "Large", vcpu: 8, ram_gb: 16.0, mult: 2.0 },
    Size { id: "xlarge", label: "Extra large", vcpu: 16, ram_gb: 32.0, mult: 50.0 / 9.0 },
];
pub fn of(id: &str) -> Option<&'static Size> { SIZES.iter().find(|s| s.id == id) }
pub fn mult(id: &str) -> f64 { of(id).map_or(1.0, |s| s.mult) }

/// What the always-on hub uses, in GB, low and high: the hub, the agents of the bots while they run, local dictation when it is on, and the
/// system's own share. `agents`: how many agent processes may run at once there (the uberbot and a bot or two).
pub fn need_gb(dictation: bool, agents: u32) -> (f64, f64) {
    let (hub, sys) = ((0.2, 0.5), (0.4, 0.6));
    let agent = (0.25, 0.5);
    let voice = if dictation { (0.7, 1.2) } else { (0.0, 0.0) };
    let a = agents as f64;
    (hub.0 + sys.0 + agent.0 * a + voice.0, hub.1 + sys.1 + agent.1 * a + voice.1)
}
/// The smallest size whose RAM holds the high estimate with 15% to spare, among those the account may make.
pub fn pick(dictation: bool, agents: u32, allowed: &[String]) -> &'static str {
    let (_, hi) = need_gb(dictation, agents);
    SIZES.iter().filter(|s| allowed.is_empty() || allowed.iter().any(|a| a == s.id)).find(|s| s.ram_gb >= hi * 1.15).map_or("default", |s| s.id)
}
pub fn month_dollars(rate_default: f64, id: &str) -> f64 { rate_default * mult(id) * MONTH_H }
fn gb(x: f64) -> String { if (x - x.round()).abs() < 0.05 { format!("{}", x.round()) } else { format!("{x:.1}") } }

/// The size list the setup and Settings show: `{id, label, line, month, fits, picked}` each. "Small: 2 vCPU, 4 GB, about $13 a month on your Boat plan."
pub fn list(rate_default: f64, dictation: bool, agents: u32, allowed: &[String], chosen: &str) -> Vec<Value> {
    let rate = if rate_default > 0.0 { rate_default } else { DEFAULT_RATE };
    let (_, hi) = need_gb(dictation, agents);
    let pick = if chosen.is_empty() || of(chosen).is_none() { pick(dictation, agents, allowed) } else { chosen };
    SIZES.iter().filter(|s| allowed.is_empty() || allowed.iter().any(|a| a == s.id)).map(|s| {
        let m = month_dollars(rate, s.id);
        json!({"id": s.id, "label": s.label, "month": (m * 100.0).round() / 100.0, "fits": s.ram_gb >= hi * 1.15, "picked": s.id == pick,
            "line": format!("{}: {} vCPU, {} GB, about ${:.0} a month on your Boat plan.", s.label, s.vcpu, gb(s.ram_gb), m.round())})
    }).collect()
}
/// The RAM line beside the list: "Expected use: 1.6 to 2.9 GB of memory: the hub, the agents while they run, local dictation."
pub fn need_line(dictation: bool, agents: u32) -> String {
    let (lo, hi) = need_gb(dictation, agents);
    format!("Expected use: {lo:.1} to {hi:.1} GB of memory: the hub 0.2 to 0.5 GB, the agents of your bots while they run{}, and the system.", if dictation { ", local dictation 0.7 to 1.2 GB" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn small_is_about_thirteen_dollars_a_month_and_xlarge_is_boats_multiple() {
        assert_eq!(month_dollars(0.036, "small").round(), 13.0);
        assert_eq!(month_dollars(0.036, "default").round(), 26.0);
        assert!((month_dollars(0.036, "xlarge") / MONTH_H - 0.2).abs() < 0.001, "xlarge is $0.20 an hour");
        let l = list(0.036, true, 2, &[], "");
        assert_eq!(l[0]["line"], "Small: 2 vCPU, 4 GB, about $13 a month on your Boat plan.");
    }
    #[test] fn the_smallest_size_that_fits_is_preselected_and_dictation_counts() {
        let (lo, hi) = need_gb(true, 2);
        assert!(lo >= 1.5 && hi <= 3.5, "{lo} {hi}");
        assert_eq!(pick(true, 2, &[]), "small", "about 3 GB fits in 4 with 15% to spare");
        assert_eq!(pick(true, 6, &[]), "default", "six agents at once need more");
        assert_eq!(pick(false, 1, &["default".into(), "large".into()]), "default", "only what the account may make");
        let l = list(0.036, true, 2, &[], ""); assert_eq!(l.iter().filter(|s| s["picked"] == true).count(), 1); assert_eq!(l[0]["picked"], true);
        assert_eq!(list(0.036, true, 2, &[], "large")[2]["picked"], true, "the person's choice stays");
        assert!(need_line(true, 2).contains("local dictation 0.7 to 1.2 GB") && !need_line(false, 2).contains("dictation"));
    }
}
