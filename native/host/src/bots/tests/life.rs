//! Ephemeral rooms (bots/life.rs) against a real Hub: a bots' exchange is listed to nobody, read and posted by its participants, closed at the
//! conversation cap or a day idle, read by its id after, and kept by the person. Stand-ins only.
use super::*;

fn thread_of(hub: &Arc<Hub>, b: &str) -> String { log(hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b).unwrap()["thread"].as_str().unwrap().to_string() }
fn names(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap().to_string()).collect() }

#[tokio::test] async fn a_topic_room_is_an_exchange_listed_to_nobody() {
    let (hub, bots, rec) = start("life-topic", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let (ta, tb) = (thread_of(&hub, &ada), thread_of(&hub, &bob));
    rpc(&hub, "bots.room", json!({"name":"staff","members":format!("{ada}\n{bob}")})).await;
    let (ok, r) = bots.tool_call(&ta, "bot_send", &json!({"to":"bob","text":"is sign-in fixed?","topic":"Google sign-in check"})).await;
    assert!(ok, "{r}"); let room = r["room"].as_str().unwrap().to_string();
    assert!(r["note"].as_str().unwrap().contains("listed to nobody"), "{r}");
    let made = log(&hub).into_iter().find(|e| e["$"] == "RoomSet" && e["id"] == room.as_str()).unwrap();
    assert_eq!(made["ephemeral"], true, "bot_send's room is for the exchange");
    assert!(rec.got.lock().unwrap().iter().any(|g| g.0 == tb), "bob hears it");
    // no list shows it: room_list, bot_whoami, plane_read, the phone's bots.list, the sidebar
    let (_, l) = bots.tool_call(&tb, "room_list", &json!({})).await; assert_eq!(names(&l), ["staff"]);
    let (_, me) = bots.tool_call(&tb, "bot_whoami", &json!({})).await; assert_eq!(names(&me["rooms"]), ["staff"]);
    let (_, plane) = bots.tool_call(&tb, "plane_read", &json!({})).await; assert!(!plane["text"].as_str().unwrap().contains("Google sign-in"), "{plane}");
    { let w = bots.world().await; let (_, rows) = crate::bots::api::rows_of(&w, "", &Value::Null); assert_eq!(names(&Value::Array(rows)), ["staff"]); }
    let mut st = crate::state::State::default(); for l in hub.store.range(0, usize::MAX).unwrap() { st.apply_line(&l); }
    assert!(st.rooms_nav[&room].ephemeral && !st.rooms_nav[&room].closed);
    // its participants reach it by topic and by id; the same topic goes on in it
    let (ok, p) = bots.tool_call(&tb, "room_post", &json!({"room":"Google sign-in check","text":"yes"})).await; assert!(ok, "{p}");
    let (ok, p) = bots.tool_call(&tb, "room_post", &json!({"room":room,"text":"since 9"})).await; assert!(ok, "{p}");
    let (_, again) = bots.tool_call(&ta, "bot_send", &json!({"to":"bob","text":"thanks","topic":"Google sign-in check"})).await; assert_eq!(again["room"], room.as_str());
    let (_, read) = bots.tool_call(&ta, "room_read", &json!({"room":room})).await; assert_eq!(read.as_array().unwrap().len(), 4);
    // a bot not in it cannot find it by its topic
    let cy = bot(&hub, "cy").await; let tc = thread_of(&hub, &cy);
    let (ok, _) = bots.tool_call(&tc, "room_post", &json!({"room":"Google sign-in check","text":"me too"})).await; assert!(!ok);
    // room_create: an exchange by default, permanent with keep (the person asked)
    let (_, c) = bots.tool_call(&ta, "room_create", &json!({"name":"quick","members":"bob"})).await;
    let (_, k) = bots.tool_call(&ta, "room_create", &json!({"name":"lasting","members":"bob","keep":true})).await;
    assert!(c["room"].is_string() && k["room"].is_null(), "an exchange's id comes back to post by; a lasting room answers as before");
    let life = |n: &str| log(&hub).into_iter().find(|e| e["$"] == "RoomSet" && e["name"] == n).unwrap()["ephemeral"].clone();
    assert_eq!((life("quick"), life("lasting")), (json!(true), json!(false)));
    let (_, l) = bots.tool_call(&tb, "room_list", &json!({})).await; assert_eq!(names(&l), ["staff", "lasting"]);
}

#[tokio::test] async fn an_exchange_closes_at_the_cap_and_is_read_by_its_id() {
    let (hub, bots, _rec) = start("life-cap", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let (ta, tb) = (thread_of(&hub, &ada), thread_of(&hub, &bob));
    let mut room = String::new();
    let cap = crate::bots::gate::LIMITS.chatter as u64; // the chatter guard closes an exchange (it replaced the 6-hop cap)
    for i in 1..=cap {
        let (from, to) = if i % 2 == 1 { (&ta, "bob") } else { (&tb, "ada") };
        let (ok, r) = bots.tool_call(from, "bot_send", &json!({"to":to,"text":format!("m{i}"),"topic":"pebble check"})).await; assert!(ok, "{r}");
        if i == cap { assert_eq!((r["held"].clone(), r["closed"].clone()), (json!(true), json!(true)), "{r}") }
        else if i == 1 { room = r["room"].as_str().unwrap().to_string() } else { assert_eq!(r["room"], room.as_str(), "message {i}") }
    }
    { let w = bots.world().await; let r = &w.rooms[&room]; assert!(r.ephemeral && r.closed); }
    let (ok, p) = bots.tool_call(&ta, "room_post", &json!({"room":room,"text":"one more"})).await; assert!(!ok && p["error"].as_str().unwrap().contains("closed"), "{p}");
    let (ok, read) = bots.tool_call(&tb, "room_read", &json!({"room":room})).await; assert!(ok); assert_eq!(read.as_array().unwrap().len(), cap as usize + 1, "kept for history, read by id: the posts and the hub's note that it paused");
    // the same topic later is a new exchange in a new room
    let (_, r) = bots.tool_call(&ta, "bot_send", &json!({"to":"bob","text":"new day","topic":"pebble check"})).await;
    assert_ne!(r["room"], room.as_str());
}

#[tokio::test] async fn an_idle_exchange_closes_and_the_person_keeps_one() {
    let (hub, bots, _rec) = start("life-idle", "").await;
    let (ada, _bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let ta = thread_of(&hub, &ada);
    let (_, a) = bots.tool_call(&ta, "bot_send", &json!({"to":"bob","text":"x","topic":"old"})).await; let old = a["room"].as_str().unwrap().to_string();
    let (_, b) = bots.tool_call(&ta, "bot_send", &json!({"to":"bob","text":"y","topic":"kept"})).await; let kept = b["room"].as_str().unwrap().to_string();
    // the first tick classifies what came before (nothing here: these carry their life) and marks it done
    bots.rooms_due(now_s()).await;
    assert!(log(&hub).iter().any(|e| e["$"] == "SettingSet" && e["key"] == crate::bots::life::CLASSIFIED));
    { let w = bots.world().await; assert!(!w.rooms[&old].closed && !w.rooms[&kept].closed, "a fresh exchange stays open"); }
    // the person keeps one: permanent, listed, offered no Keep again
    assert_eq!(rpc(&hub, "bots.room_keep", json!({"room":kept})).await["ok"], true);
    bots.rooms_due(now_s() + crate::bots::life::IDLE + 1).await;
    let w = bots.world().await;
    assert!(w.rooms[&old].closed, "a day idle closes it");
    assert!(w.rooms[&kept].listed() && w.rooms[&kept].live(), "a kept room never closes");
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "SettingSet" && e["key"] == crate::bots::life::CLASSIFIED).count(), 1, "classified once");
}

#[tokio::test] async fn the_rooms_from_before_are_closed_once_at_the_first_tick() {
    // the Nov 11 home in small: a topic room of two bots from before the field, #staff and ring-requests
    let lines = [
        json!({"$":"BotSet","id":"b1","name":"Bub","thread":"t1","look":1,"persona":"","at":1}), json!({"$":"BotSet","id":"b2","name":"Calendar","thread":"t2","look":1,"persona":"","at":1}),
        json!({"$":"BotSet","id":"b3","name":"Pebble","thread":"t3","look":1,"persona":"","at":1}),
        json!({"$":"RoomSet","id":"r1794329368-554562","name":"Google sign-in check","members":"b1\nb2","at":2}),
        json!({"$":"RoomSet","id":"r2","name":"staff","members":"b1\nb2\nb3","at":2}),
        json!({"$":"RoomSet","id":"r3","name":"ring-requests","members":"b3\nb1","at":2})];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let (hub, bots, _rec) = start("life-migrate", &text).await;
    bots.rooms_due(now_s()).await;
    let w = bots.world().await;
    assert!(w.rooms["r1794329368-554562"].ephemeral && w.rooms["r1794329368-554562"].closed);
    assert!(w.rooms["r2"].listed() && w.rooms["r3"].listed());
    drop(w);
    let mut st = crate::state::State::default(); for l in hub.store.range(0, usize::MAX).unwrap() { st.apply_line(&l); }
    assert!(st.rooms_nav["r1794329368-554562"].ephemeral && !st.rooms_nav["r2"].ephemeral);
}
