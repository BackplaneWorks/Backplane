//! Browser data/1 requests are authorized before dispatch, including forwarded requests.
use serde_json::Value;

const ALLOWED: &[&str] = &[
    "ask.answer",
    "attachment.get",
    "bot.say",
    "bots.list",
    "dirs.list",
    "fab.profile",
    "files.entries",
    "find",
    "history.list",
    "link.resolve",
    "locks.list",
    "mech.list",
    "mentions.list",
    "message.image",
    "message.send",
    "models.list",
    "project.add",
    "project.card",
    "projects.list",
    "review.list",
    "review.page",
    "room.post",
    "room.posts",
    "search",
    "sidebar.sections",
    "subscribe",
    "swarm.get",
    "swarm.list",
    "swarm.post",
    "term.list",
    "term.new",
    "term.read",
    "term.resize",
    "term.stop",
    "term.unwatch",
    "term.watch",
    "term.write",
    "thread.create",
    "thread.entries",
    "thread.order",
    "thread.pick",
    "thread.picker",
    "thread.queue",
    "thread.review",
    "thread.sub",
    "thread.work",
    "threads.list",
    "turn.interrupt",
    "unsubscribe",
    "viewer.compare",
    "viewer.diff",
    "viewer.files",
    "viewer.info",
    "viewer.layers",
    "viewer.probe",
    "viewer.refs",
    "viewer.watch",
];

#[cfg(test)]
const RESTRICTED: &[&str] = &[
    "account.act",
    "account.view",
    "accounts.page",
    "app.update",
    "badge.list",
    "bot.cat",
    "bot.delete",
    "browser",
    "cat.rig",
    "device.drop",
    "device.prefs",
    "device.register",
    "devices.page",
    "devices.page.act",
    "hub.offer.done",
    "hub.offer.take",
    "improvements.project",
    "improvements.projects",
    "kicad.checks.fix",
    "kicad.checks.focus",
    "kicad.checks.list",
    "kicad.checks.run",
    "kicad.checks.watch",
    "machines.page",
    "machines.page.act",
    "mcp.page",
    "mcp.page.act",
    "mgmt.bom.alts",
    "mgmt.bom.edit",
    "mgmt.life.part",
    "mgmt.panel",
    "mgmt.watch",
    "notify.page",
    "notify.page.act",
    "onboard.act",
    "onboard.view",
    "order.buy",
    "plan.approve",
    "plan.focus",
    "plan.stop",
    "project.act",
    "push.prefs",
    "push.seen",
    "push.state",
    "queue.act",
    "review.comment",
    "review.tick",
    "ring.get",
    "ring.hear",
    "ring.list",
    "ring.page",
    "ring.page.act",
    "ring.search",
    "ring.send",
    "ring.setup",
    "room.delete",
    "room.keep",
    "secret.asks",
    "secret.dismiss",
    "secret.set",
    "sidebar.act",
    "star.nudge",
    "star.offer",
    "swarm.act",
    "swarm.create",
    "telemetry.notice",
    "telemetry.notice.act",
    "thread.act",
    "thread.buy",
    "thread.model",
    "update.channel",
    "update.check",
    "update.now",
    "update.status",
    "voice.act",
    "voice.always",
    "voice.key",
    "voice.page",
    "voice.page.act",
    "voice.start",
    "voice.stop",
    "voice.view",
    "work.act",
];

pub fn allowed(method: &str, params: &Value) -> bool {
    if method == "composer.run" {
        use crate::session::composer::{word_of, Word};
        return matches!(word_of(params["text"].as_str().unwrap_or("")), Some(Word::Btw(_) | Word::Settle | Word::Intent(_) | Word::Project(_) | Word::Give));
    }
    ALLOWED.contains(&method)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dispatched_method_has_a_browser_policy() {
        for (source, signature) in [
            (include_str!("mod.rs"), "async fn call("),
            (include_str!("mod.rs"), "async fn handle("),
            (include_str!("ext.rs"), "pub async fn call("),
            (include_str!("term.rs"), "pub async fn call("),
        ] {
            let body = source.split_once(signature).unwrap().1.split_once("\n}").unwrap().0;
            check_dispatch(body);
        }
        assert!(!allowed("unregistered", &Value::Null));
    }

    fn check_dispatch(body: &str) {
        for line in body.lines().filter_map(|s| s.strip_prefix("        \"")) {
            let Some((patterns, _)) = line.split_once("=>") else { continue };
            let patterns = patterns.split(" if ").next().unwrap();
            for part in patterns.split('|') {
                let method = part.trim().trim_matches('"');
                assert!(method == "composer.run" || ALLOWED.contains(&method) || RESTRICTED.contains(&method), "unclassified method {method}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "unclassified method example.new")]
    fn adding_an_unclassified_rpc_fails_the_check() {
        check_dispatch(r#"        "example.new" => Ok(json!({}))"#);
    }

}
