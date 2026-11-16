//! Cat rigs for the phones (data/1 `cat.rig`): the twelve coats' rigs in every mood are built into the hub (assets/cat-rigs, 0..11; a look's
//! other collars differ only in the collar's fill), and a rig is answered with its collar and the bot's colour already applied (bots/cat.rs),
//! so a phone draws exactly what the desktop draws with no hash or tint of its own. The desktop client reads the same files beside itself.
use super::cat;
use serde_json::{json, Value};

pub const MOODS: [&str; 8] = ["away", "error", "idle", "sleep", "talk", "think", "wait", "work"];
macro_rules! coat { ($c:literal) => { [
    include_str!(concat!("../../../../assets/cat-rigs/", $c, "-away.json")), include_str!(concat!("../../../../assets/cat-rigs/", $c, "-error.json")),
    include_str!(concat!("../../../../assets/cat-rigs/", $c, "-idle.json")), include_str!(concat!("../../../../assets/cat-rigs/", $c, "-sleep.json")),
    include_str!(concat!("../../../../assets/cat-rigs/", $c, "-talk.json")), include_str!(concat!("../../../../assets/cat-rigs/", $c, "-think.json")),
    include_str!(concat!("../../../../assets/cat-rigs/", $c, "-wait.json")), include_str!(concat!("../../../../assets/cat-rigs/", $c, "-work.json")),
] } }
static RIGS: [[&str; 8]; 12] = [coat!("0"), coat!("1"), coat!("2"), coat!("3"), coat!("4"), coat!("5"), coat!("6"), coat!("7"), coat!("8"), coat!("9"), coat!("10"), coat!("11")];

/// The rig of `look` in `mood` (an unknown mood is idle), its fur in `color` when one is given: the parts as the files have them
/// (`name`, `d`, `fill`, `stroke`, `sw`, `alpha`, `anims`), back to front.
/// Away greys the whole cat: with a colour, a part's colours are its idle ones tinted, then greyed (`cat::grey`).
pub fn rig(look: u64, mood: &str, color: Option<u32>) -> Value {
    let m = MOODS.iter().position(|x| *x == mood).unwrap_or(2);
    let coat = RIGS[cat::coat(look) as usize];
    let mut parts: Vec<Value> = serde_json::from_str(coat[m]).unwrap_or_default();
    let idle: Vec<Value> = if m == 0 && color.is_some() { serde_json::from_str(coat[2]).unwrap_or_default() } else { vec![] };
    let num = |v: &Value, k: &str| v[k].as_u64().map(|x| x as u32);
    let body = idle.iter().chain(parts.iter()).find(|p| p["name"] == "body").and_then(|p| num(p, "fill")).unwrap_or(0);
    for p in parts.iter_mut() {
        let name = p["name"].as_str().unwrap_or("").to_string();
        if name == "collar" { p["fill"] = json!(if m == 0 { cat::AWAY_COLLAR } else { cat::COLLAR }[cat::collar(look) as usize]); continue }
        if m == 0 && color.is_some() {
            let Some(i) = idle.iter().find(|i| i["name"] == p["name"]) else { continue };
            let (f, s) = cat::tint(&name, num(i, "fill"), num(i, "stroke"), body, color);
            if f != num(i, "fill") { if let Some(f) = f { p["fill"] = json!(cat::grey(f)) } }
            if s != num(i, "stroke") { if let Some(s) = s { p["stroke"] = json!(cat::grey(s)) } }
            continue
        }
        let (f, s) = cat::tint(&name, num(p, "fill"), num(p, "stroke"), body, color);
        if let Some(f) = f { p["fill"] = json!(f) } if let Some(s) = s { p["stroke"] = json!(s) }
    }
    Value::Array(parts)
}

/// `cat.rig {look?, name?, color?, mood?}` → `{look, color, parts}`: by `look` when given (a bot's, from `bots.list`), else the look `name`
/// gives (a name being typed: the Settings preview). `color` "#rrggbb" or "" (the coat's own).
pub fn answer(p: &Value) -> Value {
    let look = p["look"].as_u64().unwrap_or_else(|| cat::look_of(p["name"].as_str().unwrap_or(""))) % cat::LOOKS;
    let color = p["color"].as_str().and_then(cat::parse_color);
    json!({"look": look, "color": color.map(cat::color_hex).unwrap_or_default(), "parts": rig(look, p["mood"].as_str().unwrap_or("idle"), color)})
}

/// A bot's cat from its BotSet line: (look, auto, colour). `auto`: no look was picked, so the look is the name's (`cat::look_of`, worked out
/// again here, so the line's own `look` is only for hubs that know no `auto`); a line from before `auto` keeps the look it stored.
/// `color` "#rrggbb", or "" for the coat's own.
pub fn of_event(e: &Value) -> (u64, bool, String) {
    let auto = e["auto"] == true;
    let look = if auto { cat::look_of(e["name"].as_str().unwrap_or("")) } else { e["look"].as_u64().unwrap_or(0) % cat::LOOKS };
    (look, auto, e["color"].as_str().and_then(cat::parse_color).map(cat::color_hex).unwrap_or_default())
}
/// The cat's fields of a BotSet line: `look` (the look shown, for every reader), `auto` and `color`.
pub fn fields(look: u64, auto: bool, color: &str) -> Value { json!({"look": look, "auto": auto, "color": color}) }
/// `ev` (a BotSet line) with the cat's fields set.
pub fn with(mut ev: Value, look: u64, auto: bool, color: &str) -> Value { if let (Some(o), Value::Object(f)) = (ev.as_object_mut(), fields(look, auto, color)) { o.extend(f) } ev }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn every_look_and_mood_is_built_in_and_matches_the_files() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cat-rigs");
        for look in [0u64, 13, 39, 71] { for m in MOODS {
            let file: Value = serde_json::from_str(&std::fs::read_to_string(root.join(format!("{look}-{m}.json"))).unwrap()).unwrap();
            assert_eq!(rig(look, m, None), file, "{look}-{m}: the built-in coat with its collar is the file");
        } }
    }
    #[test] fn a_colour_tints_the_fur_and_a_name_picks_the_look() {
        let a = answer(&json!({"name": "Bub", "color": "#6FB3F2"}));
        assert_eq!((a["look"].as_u64(), a["color"].as_str()), (Some(cat::look_of("Bub")), Some("#6fb3f2")));
        let body = a["parts"].as_array().unwrap().iter().find(|p| p["name"] == "body").unwrap();
        assert_eq!(body["fill"], 0x6fb3f2);
        assert_eq!(answer(&json!({"look": 75}))["look"], 3, "a stored look past 72 (the old seeds) is taken mod 72");
        assert_eq!(answer(&json!({"look": 1, "color": "nope"}))["color"], "", "a colour that does not parse is none");
    }
    #[test] fn a_line_says_whether_the_look_follows_the_name() {
        assert_eq!(of_event(&json!({"name": "Bub", "look": 3, "auto": true, "color": "#AABBCC"})), (cat::look_of("Bub"), true, "#aabbcc".into()), "auto: the name's look, whatever was stored");
        assert_eq!(of_event(&json!({"name": "Bub", "look": 4294967290u64})), (4294967290 % 72, false, String::new()), "an old line keeps its look");
        assert_eq!(with(json!({"$": "BotSet", "look": 1}), 9, false, "#000000")["look"], 9);
    }
}
