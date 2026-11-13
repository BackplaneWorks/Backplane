//! The machine switcher over the sidebar's projects: All, this machine, then each linked machine (an away one dimmer). The hub sends it as
//! `machines` only while machines are linked, decides what each shows (present/screen.rs `machine_switch`) and filters the list; a press sends
//! `side-machine` with the chip's value. A script reaches each chip as `machine:<label>`.
use crate::{hub::Command, script::{Ctl, CtlEntry}, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, PartialEq, Clone)]
pub struct Chip { pub value: String, pub label: String, pub on: bool, pub away: bool, pub hint: String }
/// The chips as the hub gives them; none when no machine is linked.
pub fn chips(machines: &Value) -> Vec<Chip> {
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    machines.as_array().into_iter().flatten().map(|m| Chip { value: s(m, "value"), label: s(m, "label"), on: m["on"].as_bool().unwrap_or(false), away: m["away"].as_bool().unwrap_or(false), hint: s(m, "hint") }).filter(|c| !c.label.is_empty()).collect()
}
pub fn switch(cx: &mut Context<Backplane>, light: bool, machines: &Value, ctl: &mut Vec<CtlEntry>) -> Option<AnyElement> {
    let cs = chips(machines);
    if cs.len() < 2 { return None }
    let c = |n: &str| theme::color(light, n);
    let mut row = div().id("machine-switch").flex().flex_wrap().items_center().gap_1().px_2().pb_2();
    for ch in cs {
        ctl.push(CtlEntry { id: format!("machine:{}", ch.label), label: ch.hint.clone(), ctl: Ctl::Node("side-machine".into(), ch.value.clone()) });
        let v = ch.value.clone();
        row = row.child(div().id(SharedString::from(format!("machine-{}", ch.label))).px_2().py(px(2.)).text_size(crate::scale::tpx(11.)).border_1().cursor_pointer()
            .border_color(if ch.on { c("accent") } else { c("line") }).text_color(if ch.on { c("text") } else { c("dim") }).when(ch.on, |d| d.bg(c("raise")))
            .when(ch.away, |d| d.opacity(0.5)).hover(|s| s.text_color(c("text")))
            .tooltip(crate::keys::tip(&ch.hint, "", light))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("side-machine".into(), v.clone())); cx.notify(); }))
            .child(ch.label));
    }
    Some(row.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn chips_are_the_hubs_and_none_without_linked_machines() {
        let m = serde_json::json!([{"value":"","label":"All","on":true,"away":false,"hint":"Every machine"},{"value":"@here","label":"desk","on":false,"away":false,"hint":"This machine"},
            {"value":"lap.ts.net","label":"lap","on":false,"away":true,"hint":"lap.ts.net · away"}]);
        let c = chips(&m); assert_eq!(c.len(), 3); assert!(c[0].on && !c[1].on); assert!(c[2].away); assert_eq!(c[2].value, "lap.ts.net");
        assert!(chips(&Value::Null).is_empty());
    }
}
