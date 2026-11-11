//! A room's life. A room the person makes, and a role room a feature sets up (#staff, ring-requests), is permanent: listed, shown, kept.
//! A room a bot makes for one exchange (bot_send's topic room, room_create) is ephemeral: no screen of the person's lists it, no bot's context
//! (room_list, bot_whoami, plane_read) shows it, and its participants post and read it by id or topic. It closes when the exchange ends:
//! at the conversation cap (tools.rs `t_post`) or after `IDLE` with no post (`rooms_due`, on the minute tick). A closed room stays in the log
//! and is read by its id. The person keeps one (`bots.room_keep`, data/1 `room.keep`, the room page's Keep): it is permanent from then on.
//! All of it is two optional fields of RoomSet, `ephemeral` and `closed` (world.rs `life`).
use super::rpc::Bots;
use super::world::{Room, World};
use serde_json::{json, Value};

/// an ephemeral room with no post or change for this long is closed
pub const IDLE: u64 = 24 * 3600;
/// the setting that says the rooms from before ephemeral rooms were classified (once)
pub const CLASSIFIED: &str = "rooms.life";

/// Life.idle: the open ephemeral rooms of this machine idle since `now - IDLE`, by id
pub fn idle<'a>(w: &'a World, now: u64) -> Vec<&'a Room> {
    let mut v: Vec<_> = w.rooms.values().filter(|r| r.live() && r.ephemeral && !r.id.contains('~') && r.last + IDLE <= now).collect(); v.sort_by(|a, b| a.id.cmp(&b.id)); v
}

/// Life.classify: what a room from before the fields is, once. A room of exactly two members, every one a bot (here or `name@machine`), is
/// the room bot_send made for one topic: ephemeral, and closed (its exchange is long over). A role room (staff, ring-requests), a room with a
/// third member or one that names no bot is the person's: it stays permanent. Linked machines' copies (`<link>~`) are their own machine's to classify.
pub fn classify(w: &World) -> Vec<&Room> {
    const ROLE: [&str; 2] = ["staff", super::ring::ROOM_NAME];
    let bot = |m: &str| m.contains('@') || w.bot(m).is_some();
    let mut v: Vec<_> = w.rooms.values().filter(|r| !r.deleted && !r.ephemeral && !r.id.contains('~') && !r.id.starts_with("dm:")
        && !ROLE.iter().any(|n| r.name.eq_ignore_ascii_case(n)) && r.members.len() == 2 && r.members.iter().all(|m| bot(m))).collect();
    v.sort_by(|a, b| a.id.cmp(&b.id)); v
}

impl Bots {
    /// The minute tick's room work: the one classification of the rooms from before, then each idle exchange closed
    pub async fn rooms_due(&self, now: u64) {
        if crate::copy::on() { return }
        let mut evs: Vec<Value> = vec![];
        let first = self.hub.setting(CLASSIFIED).await.unwrap_or_default().is_empty();
        let w = self.world().await;
        if first {
            let old = classify(&w);
            for r in &old { eprintln!("bots: room {} ({}) is a bots' exchange from before: ephemeral, closed", r.id, r.name) }
            evs.extend(old.into_iter().map(|r| super::tools::close_ev(&r.id, &r.name, &r.members, now)));
            evs.push(json!({"$":"SettingSet","key":CLASSIFIED,"value":"1","at":now}));
        }
        evs.extend(idle(&w, now).into_iter().map(|r| super::tools::close_ev(&r.id, &r.name, &r.members, now)));
        if evs.is_empty() { return }
        if let Err(e) = self.hub.commit_many(&evs).await { eprintln!("bots: rooms: {e:#}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world(lines: &[Value]) -> World { let mut w = World::default(); for l in lines { w.apply_line(&l.to_string()) } w }
    fn base() -> Vec<Value> { vec![
        json!({"$":"BotSet","id":"b1","name":"Bub","thread":"t1","look":1,"persona":"","at":1}), json!({"$":"BotSet","id":"b2","name":"Pebble","thread":"t2","look":1,"persona":"","at":1}),
        json!({"$":"BotSet","id":"b3","name":"Calendar","thread":"t3","look":1,"persona":"","at":1}),
    ] }

    /// The rooms from before: the bots' topic rooms go, the role rooms and the person's rooms stay (the Nov 11 log's rooms, in small)
    #[test] fn the_rooms_from_before_are_classified_once() {
        let mut l = base();
        l.extend([json!({"$":"RoomSet","id":"r1","name":"Google sign-in check","members":"b1\nb3","at":2}),
            json!({"$":"RoomSet","id":"r2","name":"staff","members":"b1\nb2\nb3","at":2}),
            json!({"$":"RoomSet","id":"r3","name":"ring-requests","members":"b2\nb1","at":2}),
            json!({"$":"RoomSet","id":"r4","name":"cross-machine ping","members":"Bub@p0a2\nb1","at":2}),
            json!({"$":"RoomSet","id":"r5","name":"Shop","members":"b1\nb2\nb3","at":2}),
            json!({"$":"RoomSet","id":"r6","name":"Pair","members":"b1\nb2","at":2}), json!({"$":"RoomDeleted","id":"r6","at":3}),
            json!({"$":"RoomSet","id":"L~r7","name":"theirs","members":"L~b1\nL~b2","at":2}),
            json!({"$":"RoomSet","id":"r8","name":"Staff","members":"b1\nb2","at":2})]);
        let w = world(&l);
        let ids: Vec<&str> = classify(&w).into_iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["r1", "r4"], "two bots and a topic: an exchange; a role room, a bigger room, a gone room and a linked machine's are not");
        // once closed, nothing is classified again, and a later change of members keeps the room closed
        l.push(super::super::tools::close_ev("r1", "Google sign-in check", &["b1".into(), "b3".into()], 9));
        l.push(super::super::tools::close_ev("r4", "cross-machine ping", &["Bub@p0a2".into(), "b1".into()], 9));
        l.push(json!({"$":"RoomSet","id":"r1","name":"Google sign-in check","members":"b1","at":10}));
        let w = world(&l);
        assert!(classify(&w).is_empty());
        assert!(w.rooms["r1"].ephemeral && w.rooms["r1"].closed && !w.rooms["r1"].listed() && !w.rooms["r1"].live());
        assert!(w.rooms["r2"].listed() && w.rooms["r3"].listed());
    }

    /// An exchange idle for a day closes; one with a recent post, a permanent room and a closed room do not
    #[test] fn an_idle_exchange_closes() {
        let mut l = base();
        l.extend([json!({"$":"RoomSet","id":"r1","name":"old","members":"b1\nb2","ephemeral":true,"at":100}),
            json!({"$":"RoomSet","id":"r2","name":"busy","members":"b1\nb2","ephemeral":true,"at":100}),
            json!({"$":"RoomPosted","id":"p1","room":"r2","from":"Bub","text":"x","hop":1,"at":100 + IDLE}),
            json!({"$":"RoomSet","id":"r3","name":"mine","members":"b1\nb2","at":100}),
            json!({"$":"RoomSet","id":"r4","name":"done","members":"b1\nb2","ephemeral":true,"closed":true,"at":100})]);
        let w = world(&l);
        let ids: Vec<&str> = idle(&w, 100 + IDLE).into_iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["r1"]);
        assert!(idle(&w, 99 + IDLE).is_empty());
    }

    /// Keep: a RoomSet with ephemeral false makes the room permanent and open again; a RoomSet with neither field (an old line, a join) changes neither
    #[test] fn a_kept_room_is_permanent() {
        let mut l = base();
        l.extend([json!({"$":"RoomSet","id":"r1","name":"t","members":"b1\nb2","ephemeral":true,"closed":true,"at":2}),
            json!({"$":"RoomSet","id":"r1","name":"t","members":"b1\nb2\nb3","at":3})]);
        let w = world(&l); assert!(w.rooms["r1"].ephemeral && w.rooms["r1"].closed, "a change of members keeps the life");
        l.push(json!({"$":"RoomSet","id":"r1","name":"t","members":"b1\nb2\nb3","ephemeral":false,"closed":false,"at":4}));
        let w = world(&l); assert!(w.rooms["r1"].listed() && w.rooms["r1"].live());
        // made again after a delete, a room starts permanent
        l.extend([json!({"$":"RoomSet","id":"r2","name":"x","members":"b1\nb2","ephemeral":true,"at":5}), json!({"$":"RoomDeleted","id":"r2","at":6}), json!({"$":"RoomSet","id":"r2","name":"x","members":"b1\nb2","at":7})]);
        let w = world(&l); assert!(w.rooms["r2"].listed());
    }
}
