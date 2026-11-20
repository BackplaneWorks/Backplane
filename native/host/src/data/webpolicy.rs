//! Browser data/1 requests are authorized before dispatch, including forwarded requests.
use serde_json::Value;

const ALLOWED: &[&str] = &[
    "ask.answer",
    "attachment.get",
    "badge.list",
    "bot.cat",
    "bot.delete",
    "bot.say",
    "bots.list",
    "cat.rig",
    "dirs.list",
    "fab.profile",
    "files.entries",
    "find",
    "history.list",
    "improvements.project",
    "improvements.projects",
    "kicad.checks.fix",
    "kicad.checks.focus",
    "kicad.checks.list",
    "kicad.checks.run",
    "kicad.checks.watch",
    "link.resolve",
    "locks.list",
    "mech.list",
    "mentions.list",
    "message.image",
    "message.send",
    "mgmt.bom.alts",
    "mgmt.bom.edit",
    "mgmt.life.part",
    "mgmt.panel",
    "mgmt.watch",
    "models.list",
    "plan.approve",
    "plan.focus",
    "plan.stop",
    "project.act",
    "project.add",
    "project.card",
    "projects.list",
    "queue.act",
    "review.comment",
    "review.list",
    "review.page",
    "review.tick",
    "room.delete",
    "room.keep",
    "room.post",
    "room.posts",
    "search",
    "sidebar.act",
    "sidebar.sections",
    "subscribe",
    "swarm.act",
    "swarm.create",
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
    "thread.act",
    "thread.create",
    "thread.entries",
    "thread.model",
    "thread.order",
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
    "viewer.rail",
    "viewer.refs",
    "viewer.watch",
    "voice.act",
    "voice.always",
    "voice.page",
    "voice.start",
    "voice.stop",
    "voice.view",
    "work.act",
];

#[cfg(test)]
const RESTRICTED: &[&str] = &[
    // Native provider account administration.
    "account.act",
    "account.view",
    "accounts.page",
    // Changes hub software, installation or update channel.
    "app.update",
    "update.channel",
    "update.check",
    "update.now",
    "update.status",
    // Controls the browser running on the hub machine.
    "browser",
    // Requires a native push registration or changes its delivery settings.
    "device.drop",
    "device.prefs",
    "device.register",
    "push.prefs",
    "push.seen",
    "push.state",
    "notify.page",
    "notify.page.act",
    // Native Devices can approve, revoke and change ingress; its page also renders pending comparison codes.
    "devices.page",
    "devices.page.act",
    // Controls host discovery, migration or ingress.
    "hub.offer.done",
    "hub.offer.take",
    "machines.page",
    "machines.page.act",
    "onboard.act",
    "onboard.view",
    // Displays or changes installed MCP service configuration.
    "mcp.page",
    "mcp.page.act",
    // Initiates an order or checkout.
    "order.buy",
    "thread.buy",
    // Native ring transport setup and delivery.
    "ring.get",
    "ring.hear",
    "ring.list",
    "ring.page",
    "ring.page.act",
    "ring.search",
    "ring.send",
    "ring.setup",
    // Vault or provider credential management.
    "secret.asks",
    "secret.dismiss",
    "secret.set",
    "voice.key",
    // Host-wide notification, telemetry or suggestion preferences.
    "star.nudge",
    "star.offer",
    "telemetry.notice",
    "telemetry.notice.act",
    // Native voice configuration, including model installation and settings writes.
    "voice.page.act",
];

pub fn allowed(method: &str, params: &Value) -> bool {
    // The native picker also supports switching the provider account.
    if method == "thread.pick" {
        return matches!(params["action"].as_str(), Some("provider" | "model" | "effort" | "speed" | "runtime"));
    }
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
    fn browser_can_manage_threads_without_changing_provider_credentials() {
        use serde_json::json;
        assert!(allowed("thread.act", &json!({"action":"rename"})));
        assert!(allowed("thread.pick", &json!({"action":"model"})));
        assert!(!allowed("thread.pick", &json!({"action":"account"})));
        assert!(!allowed("composer.run", &json!({"text":"/secret example"})));
        assert!(allowed("composer.run", &json!({"text":"/settle"})));
        for method in RESTRICTED { assert!(!allowed(method, &Value::Null), "{method}"); }
    }

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
                assert!(matches!(method, "composer.run" | "thread.pick") || ALLOWED.contains(&method) || RESTRICTED.contains(&method), "unclassified method {method}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "unclassified method example.new")]
    fn adding_an_unclassified_rpc_fails_the_check() {
        check_dispatch(r#"        "example.new" => Ok(json!({}))"#);
    }

}
