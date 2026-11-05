//! A post in a room (bothub.bend `Bots.post`, `Bots.fan`, `Bots.member`, `Bots.wake.room`, `Bots.direct`): the post is logged, then every other member here hears it,
//! one after the other (BotWoke, then the message). Used for a person's post (`from` empty) and for a bot's (room_post, bot_send).
use super::{mail::mailbox::room_parts, rpc::{hop_live, note, Bots}, world::World};
use serde_json::{json, Value};

/// the room a post goes to
pub struct RoomRef { pub id: String, pub name: String, pub members: Vec<String> }
/// What is left to do once the world lock is let go: the members to wake and the mail to file
pub struct Pending { wakes: Vec<(String, String, String, String)>, filing: Option<(Vec<String>, String)>, now: u64, sender: String, room: String, mid: String, text: String, from: String }

/// M.Link.id: a message id that names the thread it came from
pub fn link_id(from: &str, msg: &str) -> String { if from.is_empty() { msg.to_string() } else { format!("t:{from}:{msg}") } }

impl Bots {
    /// Log `before` (a new room), the post, and decide who hears it. The world lock stays held by the caller until this returns.
    /// `from` is the sender's thread ("" for the person), `fname` its name as the room shows it, `me` its bot id.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn post_decide(&self, w: &World, now: u64, from: &str, fname: &str, me: &str, room: &RoomRef, text: &str, hop: u64, mid: &str, before: Vec<Value>) -> Result<Pending, String> {
        let seq = self.hub.store.len() as u64;
        let mail = w.setting("mail.on", "off") == "on";
        let mut ev = before; ev.push(json!({"$":"RoomPosted","id":mid,"room":room.id,"from":fname,"text":text,"hop":hop,"at":now}));
        self.hub.commit_many(&ev).await.map_err(|e| format!("{e:#}"))?;
        let n = note(fname, &room.id, &room.name, text);
        let mut wakes = vec![];
        if !mail && hop_live(hop) {
            for (i, m) in room.members.iter().enumerate() {
                if m == me || m.contains('@') { continue } // never its sender; members elsewhere hear it through the net lane
                let Some(b) = w.bot(m).filter(|b| !b.deleted) else { continue };
                if w.triage_owner(&b.id) { continue }
                wakes.push((b.id.clone(), b.thread.clone(), link_id(from, &format!("m{now}-{seq}.{i}")), n.clone()));
            }
        }
        // B.Mail.room: with the mailroom on, or the uberbot in the room, the post is also filed once as the room's mail
        let sender = w.mail_sender(from); let ub = w.uber_bot();
        let mut parts = room_parts(&room.members); if !parts.contains(&sender) { parts.insert(0, sender.clone()) }
        let filing = if mail || (!ub.is_empty() && parts.contains(&format!("bot:{ub}"))) { Some((parts, room.name.clone())) } else { None };
        Ok(Pending { wakes, filing, now, sender, room: room.id.clone(), mid: mid.into(), text: text.into(), from: from.into() })
    }
    /// The rest of a post, with the lock let go: each member in turn is logged as woken and hears the note, then the room mail is filed.
    pub(super) async fn post_finish(&self, p: Pending, hop: u64) {
        for (bot, th, msg, text) in p.wakes {
            if self.hub.commit(&json!({"$":"BotWoke","bot":bot,"hop":hop,"at":p.now})).await.is_ok() {
                let origin = super::mail::classify(&p.from, &msg, false, &text); let _ = self.inbox_deliver_raw(&th, &msg, &text, Some((origin.from, origin.kind, origin.r#ref))).await;
            }
        }
        if let Some((parts, name)) = p.filing {
            let w = self.world().await;
            if let Err(e) = self.file_room_mail(&w, p.now, &p.sender, &p.room, &p.text, &p.mid, parts, &name).await { eprintln!("bots: could not file a mail: {e}") }
        }
    }
}
