//! Browser data/1 requests are authorized before dispatch, including forwarded requests.
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy { Passive, Activity, Devices, Denied }

/// None identifies an unclassified method in the coverage test. Dispatch denies it.
fn classify(method: &str, params: &Value) -> Option<Policy> {
    use Policy::*;
    Some(match method {
        "devices.page" => Devices,
        "thread.pick" => match params["action"].as_str() {
            Some("provider" | "model" | "effort" | "speed" | "runtime") => Activity,
            _ => Denied,
        },
        "composer.run" => {
            use crate::session::composer::{word_of, Word};
            match word_of(params["text"].as_str().unwrap_or("")) {
                Some(Word::Btw(_) | Word::Settle | Word::Intent(_) | Word::Project(_) | Word::Give) => Activity,
                _ => Denied,
            }
        }
        "ask.answer"
        | "bot.say"
        | "find"
        | "kicad.checks.focus"
        | "kicad.checks.run"
        | "message.send"
        | "mgmt.bom.edit"
        | "mgmt.panel"
        | "project.act"
        | "project.add"
        | "queue.act"
        | "review.comment"
        | "review.tick"
        | "room.keep"
        | "room.post"
        | "search"
        | "sidebar.act"
        | "swarm.act"
        | "swarm.create"
        | "swarm.post"
        | "term.new"
        | "term.resize"
        | "term.stop"
        | "term.write"
        | "thread.act"
        | "thread.again"
        | "thread.create"
        | "thread.model"
        | "thread.review"
        | "turn.interrupt"
        | "viewer.compare"
        | "viewer.follow"
        | "viewer.watch"
        | "voice.act"
        | "POST /voice/in"
        | "voice.start"
        | "voice.stop"
        | "work.act" => Activity,
        "attachment.get"
        | "badge.list"
        | "bot.cat"
        | "bot.delete"
        | "bots.list"
        | "cat.rig"
        | "dirs.list"
        | "fab.profile"
        | "files.entries"
        | "history.list"
        | "improvements.project"
        | "improvements.projects"
        | "kicad.checks.fix"
        | "kicad.checks.list"
        | "kicad.checks.watch"
        | "link.resolve"
        | "locks.list"
        | "mech.list"
        | "mentions.list"
        | "message.image"
        | "mgmt.bom.alts"
        | "mgmt.life.part"
        | "mgmt.watch"
        | "models.list"
        | "plan.approve"
        | "plan.focus"
        | "plan.stop"
        | "project.card"
        | "projects.list"
        | "review.list"
        | "review.page"
        | "room.delete"
        | "room.posts"
        | "sidebar.sections"
        | "subscribe"
        | "swarm.get"
        | "swarm.list"
        | "term.list"
        | "term.read"
        | "term.unwatch"
        | "term.watch"
        | "thread.entries"
        | "thread.order"
        | "thread.picker"
        | "thread.queue"
        | "thread.sub"
        | "thread.work"
        | "threads.list"
        | "unsubscribe"
        | "viewer.diff"
        | "viewer.files"
        | "viewer.info"
        | "viewer.layers"
        | "viewer.probe"
        | "viewer.rail"
        | "viewer.refs"
        | "voice.always"
        | "voice.page"
        | "voice.view" => Passive,
        // Native provider account administration.
        "account.act"
        | "account.view"
        | "accounts.page" => Denied,
        // Changes hub software, installation or update channel.
        "app.update"
        | "update.channel"
        | "update.check"
        | "update.now"
        | "update.status" => Denied,
        // Phone pane input. Native debugger attachment is outside this interface.
        "browser" => match params["op"].as_str() {
            Some("navigate" | "click" | "type" | "press" | "scroll" | "takeover" | "handback") => Activity,
            Some("toolbar") if matches!(params["command"].as_str(), Some("back" | "forward" | "reload")) => Activity,
            _ => Denied,
        },
        // Requires a native push registration or changes its delivery settings.
        "device.drop"
        | "device.prefs"
        | "device.register"
        | "push.prefs"
        | "push.seen"
        | "push.state"
        | "notify.page"
        | "notify.page.act" => Denied,
        // Native Devices can approve, revoke and change ingress; its page also renders pending comparison codes.
        "devices.page.act" => Denied,
        // Controls host discovery, migration or ingress.
        "hub.offer.done"
        | "hub.offer.take"
        | "machines.page"
        | "machines.page.act"
        | "onboard.act"
        | "onboard.view" => Denied,
        // Displays or changes installed MCP service configuration.
        "mcp.page"
        | "mcp.page.act" => Denied,
        // Initiates an order or checkout.
        "order.buy"
        | "thread.buy" => Denied,
        // Native ring transport setup and delivery.
        "ring.get"
        | "ring.hear"
        | "ring.list"
        | "ring.page"
        | "ring.page.act"
        | "ring.search"
        | "ring.send"
        | "ring.setup" => Denied,
        // Vault or provider credential management.
        "secret.asks"
        | "secret.dismiss"
        | "secret.set"
        | "voice.key" => Denied,
        // Host-wide notification, telemetry or suggestion preferences.
        "star.nudge"
        | "star.offer"
        | "telemetry.notice"
        | "telemetry.notice.act" => Denied,
        // Native voice configuration, including model installation and settings writes.
        "voice.page.act" => Denied,
        _ => return None,
    })
}

pub fn policy(method: &str, params: &Value) -> Policy {
    classify(method, params).unwrap_or(Policy::Denied)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn allowed(m: &str, p: &Value) -> bool { matches!(policy(m, p), Policy::Passive | Policy::Activity) }

    #[test]
    fn browser_can_manage_threads_without_changing_provider_credentials() {
        use serde_json::json;
        assert!(allowed("thread.act", &json!({"action":"rename"})));
        assert!(allowed("thread.pick", &json!({"action":"model"})));
        assert!(!allowed("thread.pick", &json!({"action":"account"})));
        assert!(!allowed("composer.run", &json!({"text":"/secret example"})));
        assert!(allowed("composer.run", &json!({"text":"/settle"})));
        assert_eq!(policy("devices.page", &Value::Null), Policy::Devices);
        assert_eq!(policy("devices.page.act", &Value::Null), Policy::Denied);
    }

    #[test]
    fn browser_pane_allows_phone_input_only() {
        use serde_json::json;
        for op in ["navigate", "click", "type", "press", "scroll", "takeover", "handback"] {
            assert_eq!(policy("browser", &json!({"op":op})), Policy::Activity);
        }
        for command in ["back", "forward", "reload"] {
            assert_eq!(policy("browser", &json!({"op":"toolbar", "command":command})), Policy::Activity);
        }
        for op in ["attach", "detach", "unknown", "toolbar", ""] {
            assert_eq!(policy("browser", &json!({"op":op})), Policy::Denied);
        }
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
                assert!(classify(method, &Value::Null).is_some(), "unclassified method {method}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "unclassified method example.new")]
    fn adding_an_unclassified_rpc_fails_the_check() {
        check_dispatch(r#"        "example.new" => Ok(json!({}))"#);
    }

}
