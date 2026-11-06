//! The microphones on this machine: PulseAudio's pactl, else PipeWire's
//! pw-dump, else ALSA's arecord -L. Monitors of outputs are not microphones.

use crate::util::which;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub label: String,
}

impl Device {
    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "label": self.label})
    }
}

#[derive(Clone, Debug, Default)]
pub struct Devices {
    pub default: String,
    pub devices: Vec<Device>,
    pub via: String,
}

fn is_monitor(name: &str, cls: &str) -> bool {
    name.ends_with(".monitor") || cls.to_lowercase().contains("monitor")
}

/// JavaScript's String(x ?? ""): a string as it is, a number or bool in
/// text, nothing as "".
pub fn text_of(v: Option<&Value>) -> Option<String> {
    match v {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(x) => Some(x.to_string()),
    }
}

/// `pactl -f json list sources`
pub fn from_pactl(json: &str) -> Result<Vec<Device>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let Some(list) = v.as_array() else { return Err("not a list".into()) };
    let mut out = vec![];
    for s in list {
        let name = text_of(s.get("name")).unwrap_or_default();
        let monitor_of = match s.get("monitor_of_sink") {
            None | Some(Value::Null) => false,
            Some(Value::String(x)) => !x.is_empty() && x != "n/a",
            Some(_) => true,
        };
        let props = s.get("properties");
        let class_monitor = props.and_then(|p| p.get("device.class")).and_then(Value::as_str) == Some("monitor");
        if name.is_empty() || is_monitor(&name, "") || monitor_of || class_monitor {
            continue;
        }
        let label = text_of(s.get("description")).or_else(|| text_of(props.and_then(|p| p.get("device.description")))).unwrap_or_else(|| name.clone());
        out.push(Device { name, label });
    }
    Ok(out)
}

/// `pw-dump`: Audio/Source nodes, and the default from the "default" metadata
pub fn from_pw_dump(json: &str) -> Result<(String, Vec<Device>), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let Some(list) = v.as_array() else { return Err("not a list".into()) };
    let mut out = vec![];
    let mut def = String::new();
    for o in list {
        let p = o.get("info").and_then(|i| i.get("props"));
        let cls = text_of(p.and_then(|p| p.get("media.class"))).unwrap_or_default();
        if let Some(name) = text_of(p.and_then(|p| p.get("node.name"))).filter(|n| !n.is_empty()) {
            if cls.starts_with("Audio/Source") && !is_monitor(&name, &cls) {
                let p = p.unwrap();
                let label = text_of(p.get("node.description")).or_else(|| text_of(p.get("node.nick"))).unwrap_or_else(|| name.clone());
                out.push(Device { name, label });
            }
        }
        if o.get("type").and_then(Value::as_str) == Some("PipeWire:Interface:Metadata") {
            for m in o.get("metadata").and_then(Value::as_array).into_iter().flatten() {
                let key = m.get("key").and_then(Value::as_str);
                let name = text_of(m.get("value").and_then(|x| x.get("name")));
                if key == Some("default.audio.source") {
                    def = name.unwrap_or(def);
                } else if key == Some("default.configured.audio.source") && def.is_empty() {
                    def = name.unwrap_or_default();
                }
            }
        }
    }
    Ok((def, out))
}

/// `arecord -L`: a name at the start of a line, its description indented below
pub fn from_arecord(text: &str) -> Vec<Device> {
    let mut out: Vec<Device> = vec![];
    for line in text.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            out.push(Device { name: line.trim().into(), label: String::new() });
        } else if let Some(d) = out.last_mut() {
            d.label = if d.label.is_empty() { line.trim().into() } else { format!("{}, {}", d.label, line.trim()) };
        }
    }
    out.retain(|d| d.name != "null");
    out
}

async fn run(cmd: &[&str]) -> Option<String> {
    let exe = which(cmd[0])?;
    let child = tokio::process::Command::new(exe)
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let out = tokio::time::timeout(Duration::from_secs(3), child.wait_with_output()).await.ok()?.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Never fails: an empty list when nothing answers.
pub async fn list_devices() -> Devices {
    if let Some(pa) = run(&["pactl", "-f", "json", "list", "sources"]).await {
        if let Ok(devices) = from_pactl(&pa) {
            let default = run(&["pactl", "get-default-source"]).await.unwrap_or_default().trim().to_string();
            return Devices { default, devices, via: "pactl".into() };
        }
    }
    if let Some(pw) = run(&["pw-dump"]).await {
        if let Ok((default, devices)) = from_pw_dump(&pw) {
            return Devices { default, devices, via: "pw-dump".into() };
        }
    }
    if let Some(al) = run(&["arecord", "-L"]).await {
        let devices = from_arecord(&al);
        let default = if devices.iter().any(|d| d.name == "default") { "default".to_string() } else { String::new() };
        return Devices { default, devices, via: "arecord".into() };
    }
    Devices::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(name: &str, label: &str) -> Device {
        Device { name: name.into(), label: label.into() }
    }

    #[test]
    fn pactl_skips_monitors() {
        let j = json!([
            {"name": "alsa_input.usb-mic.mono", "description": "USB Mic Mono", "monitor_of_sink": null, "properties": {}},
            {"name": "alsa_output.hdmi.monitor", "description": "Monitor of HDMI", "monitor_of_sink": "alsa_output.hdmi", "properties": {"device.class": "monitor"}},
            {"name": "alsa_input.webcam.stereo", "description": "Webcam Stereo", "monitor_of_sink": "n/a", "properties": {}},
            {"name": "virtual", "monitor_of_sink": "sink", "properties": {}},
            {"name": "by-class", "monitor_of_sink": null, "properties": {"device.class": "monitor"}},
            {"name": "only-props", "properties": {"device.description": "From Props"}},
            {"name": "bare"}
        ]);
        assert_eq!(
            from_pactl(&j.to_string()).unwrap(),
            vec![d("alsa_input.usb-mic.mono", "USB Mic Mono"), d("alsa_input.webcam.stereo", "Webcam Stereo"), d("only-props", "From Props"), d("bare", "bare")]
        );
        assert!(from_pactl("not json").is_err());
    }

    #[test]
    fn pw_dump_finds_sources_and_the_default() {
        let j = json!([
            {"type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Source", "node.name": "mic", "node.description": "The Mic"}}},
            {"type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "spk", "node.description": "Speakers"}}},
            {"type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Source/Virtual", "node.name": "x.monitor"}}},
            {"type": "PipeWire:Interface:Metadata", "metadata": [{"key": "default.audio.source", "value": {"name": "mic"}}]}
        ]);
        assert_eq!(from_pw_dump(&j.to_string()).unwrap(), ("mic".to_string(), vec![d("mic", "The Mic")]));
        let configured = json!([{"type": "PipeWire:Interface:Metadata", "metadata": [{"key": "default.configured.audio.source", "value": {"name": "m2"}}]}]);
        assert_eq!(from_pw_dump(&configured.to_string()).unwrap().0, "m2");
    }

    #[test]
    fn arecord_joins_descriptions_and_drops_null() {
        assert_eq!(
            from_arecord("null\n    Discard all samples\ndefault\n    Default ALSA Output\nhw:CARD=X,DEV=0\n    X, USB Audio\n    Direct\n"),
            vec![d("default", "Default ALSA Output"), d("hw:CARD=X,DEV=0", "X, USB Audio, Direct")]
        );
    }
}
