//! `backplane pair ...`: pair devices and machines with the hub on this computer, over ssh or at the terminal. It talks to the
//! hub on loopback with the hub's own token (read from its home, never printed): `POST /bots/pair/local`.
use crate::{http, store};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};

pub const USAGE: &str = "usage: backplane pair                          a link, a short code and a QR code for a new phone or computer
       backplane pair --machine                make a code another machine's hub enters to ask to pair with this one
       backplane pair --join ADDRESS CODE      ask the hub at ADDRESS (http://host:port) to pair, with the code it made
       backplane pair --requests               machines waiting for you to approve, and your own attempts
       backplane pair --approve ID             approve a request (after checking its check code matches the other machine's)
       backplane pair --deny ID                refuse it
Which hub: --home DIR, else $BACKPLANE_HOME, else the one started with --default; --port N overrides the port the hub's home names.
";

pub struct Opts { pub home: Option<PathBuf>, pub port: Option<u16>, pub mode: Mode }
#[derive(Debug, PartialEq)]
pub enum Mode { Device, Machine, Join(String, String), Requests, Approve(String), Deny(String) }

/// The command line after `pair`. Err is the text to say before exit 2.
pub fn parse(a: &[String]) -> Result<Opts, String> {
    let (mut home, mut port, mut mode) = (None, None, Mode::Device);
    let mut i = 0;
    let mut next = |i: &mut usize| -> Result<String, String> { *i += 1; a.get(*i).cloned().ok_or_else(|| USAGE.to_string()) };
    while i < a.len() {
        match a[i].as_str() {
            "--home" => home = Some(next(&mut i)?.into()),
            "--port" => port = Some(next(&mut i)?.parse::<u16>().map_err(|_| USAGE.to_string())?),
            "--machine" => mode = Mode::Machine,
            "--requests" => mode = Mode::Requests,
            "--join" => { let (x, y) = (next(&mut i)?, next(&mut i)?); mode = Mode::Join(x, y) }
            "--approve" => mode = Mode::Approve(next(&mut i)?),
            "--deny" => mode = Mode::Deny(next(&mut i)?),
            "-h" | "--help" => return Err(USAGE.to_string()),
            _ => return Err(USAGE.to_string()),
        }
        i += 1;
    }
    Ok(Opts { home, port, mode })
}

/// The home to talk to, and the port its hub listens on (hub.lock says "<pid> <port> flock"; the guard's status has it too).
pub fn locate(o: &Opts) -> Result<(PathBuf, u16), String> {
    let home = o.home.clone().or_else(|| std::env::var_os("BACKPLANE_HOME").map(PathBuf::from))
        .or_else(|| store::pointer().and_then(|p| fs::read_to_string(p).ok()).map(|s| PathBuf::from(s.trim())))
        .ok_or("which hub? --home DIR, or BACKPLANE_HOME")?;
    let port = o.port.or_else(|| port_of(&home)).ok_or_else(|| format!("no hub seems to run for {}: start it, or give --port", home.display()))?;
    Ok((home, port))
}
fn port_of(home: &std::path::Path) -> Option<u16> {
    let lock = fs::read_to_string(home.join("hub.lock")).ok().and_then(|t| t.split_whitespace().nth(1).and_then(|p| p.parse().ok()));
    lock.or_else(|| store::Dir::of(home).json("status.json")["port"].as_u64().map(|p| p as u16))
}

fn ask(home: &std::path::Path, port: u16, body: Value) -> Result<Value, String> {
    let token = fs::read_to_string(home.join("token")).map_err(|_| format!("cannot read the hub token in {}", home.display()))?.trim().to_string();
    match http::call(&format!("127.0.0.1:{port}"), "POST", "/bots/pair/local", &token, &body.to_string(), Duration::from_secs(15)) {
        None => Err(format!("the hub does not answer on 127.0.0.1:{port}")),
        Some((200, b)) => { let j: Value = serde_json::from_str(&b).map_err(|_| "the hub answered nothing usable".to_string())?; if j["ok"] == false { Err(j["error"].as_str().unwrap_or("refused").to_string()) } else { Ok(j) } }
        Some((401, _)) => Err("the hub refused its own token: is this the right --home?".into()),
        Some((c, b)) => Err(format!("{c} {}", b.trim())),
    }
}

/// A QR code for a terminal: two module rows per text row, with a two-module quiet zone. Light modules are the filled ones, which
/// reads on a dark terminal (a light one inverts it).
pub fn qr_text(rows: &str) -> String {
    let m: Vec<Vec<bool>> = rows.lines().filter(|l| !l.is_empty()).map(|l| l.chars().map(|c| c == '1').collect()).collect();
    let w = m.iter().map(Vec::len).max().unwrap_or(0);
    let dark = |y: i32, x: i32| y >= 0 && x >= 0 && (y as usize) < m.len() && m.get(y as usize).and_then(|r| r.get(x as usize)).copied().unwrap_or(false);
    let mut out = String::new();
    let mut y = -2;
    while y < m.len() as i32 + 2 {
        for x in -2..w as i32 + 2 {
            out.push(match (dark(y, x), dark(y + 1, x)) { (false, false) => '█', (true, true) => ' ', (false, true) => '▀', (true, false) => '▄' });
        }
        out.push('\n'); y += 2;
    }
    out
}

fn wait_for(home: &std::path::Path, port: u16, id: &str) -> i32 {
    let t0 = store::now();
    while store::now() - t0 < 140 {
        std::thread::sleep(Duration::from_secs(1));
        match ask(home, port, json!({"op": "status", "id": id})) {
            Ok(j) => match j["state"].as_str().unwrap_or("") {
                "linked" => { println!("Paired with {}.", j["name"].as_str().unwrap_or("the other machine")); return 0 }
                "refused" => { eprintln!("Not paired: {}", j["error"].as_str().unwrap_or("refused")); return 1 }
                _ => {}
            },
            Err(e) => { eprintln!("{e}"); return 1 }
        }
    }
    eprintln!("Not approved in time."); 1
}

pub fn run(a: &[String]) -> i32 {
    let o = match parse(a) { Ok(o) => o, Err(t) => { eprint!("{t}"); return 2 } };
    let (home, port) = match locate(&o) { Ok(x) => x, Err(e) => { eprintln!("{e}"); return 1 } };
    let fail = |e: String| { eprintln!("{e}"); 1 };
    match &o.mode {
        Mode::Device => match ask(&home, port, json!({"op": "device"})) {
            Ok(j) => {
                print!("{}", qr_text(j["qr"].as_str().unwrap_or("")));
                println!("Link: {}\nCode: {} (with the hub's address; works once, for ten minutes)", j["link"].as_str().unwrap_or(""), j["code"].as_str().unwrap_or(""));
                if j["localOnly"] == true { println!("Only this computer can reach that address: start the hub with Tailscale on to pair another device.") }
                0
            }
            Err(e) => fail(e),
        },
        Mode::Machine => match ask(&home, port, json!({"op": "machine"})) {
            Ok(j) => {
                let code = j["code"].as_str().unwrap_or("");
                let list: Vec<&str> = j["addresses"].as_array().into_iter().flatten().filter_map(|a| a.as_str()).collect();
                let list = if list.is_empty() { vec![j["address"].as_str().unwrap_or("")] } else { list };
                println!("Code: {code}");
                println!("{}", if list.len() > 1 { "Addresses another machine can use, best first:" } else { "Address:" });
                for a in &list { println!("  {a}") }
                println!("On the other machine: backplane pair --join {} {code}", list[0]);
                println!("Then approve its request here: backplane pair --requests");
                if list.iter().all(|a| a.contains("//127.") || a.contains("//localhost") || a.contains("//[::1]")) { println!("Only this computer can reach that address: turn Tailscale on, or start the hub on a network address.") }
                0
            }
            Err(e) => fail(e),
        },
        Mode::Join(addr, code) => match ask(&home, port, json!({"op": "join", "address": addr, "code": code})) {
            Ok(j) => {
                if let Some(n) = j["linked"].as_str() { println!("Linked with {n} (an invite pasted in is not a pairing: mark it Mine in Settings, Machines)."); return 0 }
                println!("Check code {}: it must be the same on the other machine's request. Waiting for it to be approved there...", j["check"].as_str().unwrap_or(""));
                wait_for(&home, port, j["id"].as_str().unwrap_or(""))
            }
            Err(e) => fail(e),
        },
        Mode::Requests => match ask(&home, port, json!({"op": "requests"})) {
            Ok(j) => {
                let rs = j["requests"].as_array().cloned().unwrap_or_default();
                if rs.is_empty() { println!("No machine is waiting for you.") }
                for r in &rs { println!("{}  {}  {}  check code {}  {}s left   (backplane pair --approve {} / --deny {})", r["id"].as_str().unwrap_or(""), r["name"].as_str().unwrap_or(""), r["url"].as_str().unwrap_or(""), r["check"].as_str().unwrap_or(""), r["left"], r["id"].as_str().unwrap_or(""), r["id"].as_str().unwrap_or("")) }
                for o in j["outgoing"].as_array().into_iter().flatten() { println!("asked {}  check code {}  {}", o["address"].as_str().unwrap_or(""), o["check"].as_str().unwrap_or(""), o["status"].as_str().unwrap_or("")) }
                0
            }
            Err(e) => fail(e),
        },
        Mode::Approve(id) | Mode::Deny(id) => {
            let op = if matches!(o.mode, Mode::Approve(_)) { "approve" } else { "deny" };
            match ask(&home, port, json!({"op": op, "id": id})) { Ok(_) => { println!("{}", if op == "approve" { "Approved: the other machine links now." } else { "Refused." }); 0 } Err(e) => fail(e) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> { a.iter().map(|s| s.to_string()).collect() }
    #[test] fn the_command_line_reads() {
        assert_eq!(parse(&v(&[])).unwrap().mode, Mode::Device);
        assert_eq!(parse(&v(&["--machine"])).unwrap().mode, Mode::Machine);
        assert_eq!(parse(&v(&["--join", "http://b:1", "ABCD2345", "--port", "9"])).map(|o| (o.mode, o.port)).unwrap(), (Mode::Join("http://b:1".into(), "ABCD2345".into()), Some(9)));
        assert_eq!(parse(&v(&["--approve", "x1"])).unwrap().mode, Mode::Approve("x1".into()));
        assert_eq!(parse(&v(&["--deny", "x1", "--home", "/h"])).map(|o| (o.mode, o.home)).unwrap(), (Mode::Deny("x1".into()), Some("/h".into())));
        assert!(parse(&v(&["--join", "a"])).is_err() && parse(&v(&["--bogus"])).is_err() && parse(&v(&["--port", "x"])).is_err());
    }
    #[test] fn the_port_comes_from_the_homes_lock_or_the_override() {
        let d = std::env::temp_dir().join(format!("bp-pair-cli-{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap();
        let o = |port| Opts { home: Some(d.clone()), port, mode: Mode::Device };
        assert!(locate(&o(None)).is_err(), "no hub, no port");
        fs::write(d.join("hub.lock"), "123 3811 flock\n").unwrap();
        assert_eq!(locate(&o(None)).unwrap().1, 3811);
        assert_eq!(locate(&o(Some(4000))).unwrap().1, 4000);
        let _ = fs::remove_dir_all(&d);
    }
    #[test] fn a_qr_prints_two_rows_a_line_with_a_quiet_zone() {
        let t = qr_text("10\n01");
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines.len(), 3, "2 quiet + 2 rows + 2 quiet = 6 module rows, 3 text rows");
        assert!(lines.iter().all(|l| l.chars().count() == 6));
        assert_eq!(lines[1], "██▄▀██");
    }
}
