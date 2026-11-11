//! The gallery: a short list of servers checked to exist (2026-11-11: each page read, each remote endpoint answered an unauthenticated
//! `initialize`), one press to add. Vendor documentation first. `community`: made by someone other than the vendor, said on the card.
//! `signin`: the server wants an OAuth sign-in, which Backplane does not do yet: shown with its link, not added.
use super::model::{Kv, Server};

pub struct Entry {
    pub id: &'static str, pub name: &'static str, pub by: &'static str, pub community: bool, pub group: &'static str,
    pub about: &'static str, pub home: &'static str,
    /// stdio: the command and its args; remote: "" and the URL
    pub command: &'static str, pub args: &'static [&'static str], pub url: &'static str,
    /// a secret header or env variable it takes: (name, prefix, required)
    pub secret: Option<(&'static str, &'static str, bool)>,
    /// an argument the person must fill in (`<allowed-dir>`): its hint
    pub needs: &'static str,
    pub signin: bool,
}

const fn remote(id: &'static str, name: &'static str, by: &'static str, group: &'static str, about: &'static str, home: &'static str, url: &'static str) -> Entry {
    Entry { id, name, by, community: false, group, about, home, command: "", args: &[], url, secret: None, needs: "", signin: false }
}
const fn local(id: &'static str, name: &'static str, by: &'static str, group: &'static str, about: &'static str, home: &'static str, command: &'static str, args: &'static [&'static str]) -> Entry {
    Entry { id, name, by, community: false, group, about, home, command, args, url: "", secret: None, needs: "", signin: false }
}

pub const VENDOR: &str = "Chip vendors";
pub const DOCS: &str = "Docs and code";
pub const TOOLS: &str = "Reference servers";

pub static ENTRIES: &[Entry] = &[
    remote("microchip-docs", "Microchip MPLAB Docs", "Microchip", VENDOR, "Search Microchip MCU technical documentation", "https://www.microchip.com/en-us/resources/model-context-protocol-server", "https://ai.microchip.com/tools/mplabdocs/mcp"),
    remote("microchip-products", "Microchip Product Data", "Microchip", VENDOR, "Product specs, inventory and compliance data", "https://www.microchip.com/en-us/resources/model-context-protocol-server", "https://api.microchip.com/mcp/resources"),
    remote("esp-components", "ESP Component Registry", "Espressif", VENDOR, "ESP-IDF components and examples", "https://mcp.espressif.com/", "https://components.espressif.com/mcp"),
    Entry { community: true, ..local("stm32-docs", "STM32 Docs", "creativec09", VENDOR, "Semantic search over STM32 documentation", "https://github.com/creativec09/stm32", "uvx", &["--from", "git+https://github.com/creativec09/stm32.git", "stm32-mcp-docs"]) },
    Entry { signin: true, ..remote("espressif-docs", "Espressif Documentation", "Espressif", VENDOR, "ESP-IDF documentation (GitHub sign-in)", "https://mcp.espressif.com/", "https://mcp.espressif.com/docs") },
    Entry { signin: true, ..remote("nordic", "Nordic Semiconductor", "Nordic", VENDOR, "nRF Connect SDK and DevZone (sign-in)", "https://www.nordicsemi.com/Products/Technologies/AI-assisted-development/Get-started", "https://mcp.nordicsemi.com") },
    Entry { signin: true, ..remote("nxp-docs", "NXP Documentation", "NXP", VENDOR, "NXP product documentation (sign-in)", "https://community.nxp.com/t5/Agentic-AI-Development/Getting-Started-MCP-Server-for-Documentation/ta-p/2415278", "https://mcp.gw.ext.nxp.com/mcp") },
    Entry { signin: true, ..remote("silabs", "Silicon Labs", "Silicon Labs", VENDOR, "Silicon Labs documentation (sign-in)", "https://mcp.silabs.com/mcp", "https://mcp.silabs.com/mcp") },
    Entry { community: true, ..local("kicad-pro", "KiCad MCP Pro", "oaslananka", DOCS, "ERC, DRC, BOM and exports from KiCad", "https://github.com/oaslananka/kicad-mcp-pro", "uvx", &["kicad-mcp-pro", "--transport", "stdio"]) },
    remote("deepwiki", "DeepWiki", "Cognition", DOCS, "Docs and answers for any public GitHub repository", "https://docs.devin.ai/work-with-devin/deepwiki-mcp", "https://mcp.deepwiki.com/mcp"),
    Entry { secret: Some(("Authorization", "Bearer ", false)), ..remote("context7", "Context7", "Upstash", DOCS, "Current library docs and code examples", "https://github.com/upstash/context7", "https://mcp.context7.com/mcp") },
    Entry { secret: Some(("Authorization", "Bearer ", true)), ..remote("github", "GitHub", "GitHub", DOCS, "Repositories, issues, pull requests and Actions", "https://github.com/github/github-mcp-server", "https://api.githubcopilot.com/mcp/") },
    local("fetch", "Fetch", "MCP reference", TOOLS, "Web pages as markdown", "https://github.com/modelcontextprotocol/servers/tree/main/src/fetch", "uvx", &["mcp-server-fetch"]),
    Entry { needs: "A folder it may read and write", ..local("filesystem", "Filesystem", "MCP reference", TOOLS, "Files in the folders you allow", "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem", "npx", &["-y", "@modelcontextprotocol/server-filesystem"]) },
    local("git", "Git", "MCP reference", TOOLS, "Read and search local Git repositories", "https://github.com/modelcontextprotocol/servers/tree/main/src/git", "uvx", &["mcp-server-git"]),
    local("time", "Time", "MCP reference", TOOLS, "The time and time zone conversion", "https://github.com/modelcontextprotocol/servers/tree/main/src/time", "uvx", &["mcp-server-time"]),
    local("memory", "Memory", "MCP reference", TOOLS, "A knowledge graph that lasts between sessions", "https://github.com/modelcontextprotocol/servers/tree/main/src/memory", "npx", &["-y", "@modelcontextprotocol/server-memory"]),
    local("everything", "Everything", "MCP reference", TOOLS, "A test server with every MCP feature", "https://github.com/modelcontextprotocol/servers/tree/main/src/everything", "npx", &["-y", "@modelcontextprotocol/server-everything"]),
];

pub fn get(id: &str) -> Option<&'static Entry> { ENTRIES.iter().find(|e| e.id == id) }

impl Entry {
    pub fn remote(&self) -> bool { !self.url.is_empty() }
    /// The server it adds (`arg`: what fills `needs`), id made unique against `taken`. None for a sign-in server, or one whose `needs` is empty.
    pub fn server(&self, taken: &[String], arg: &str, now: u64) -> Option<Server> {
        if self.signin || (!self.needs.is_empty() && arg.trim().is_empty()) { return None }
        let id = super::model::unique(self.id, taken);
        let mut s = Server { id: id.clone(), name: self.name.into(), enabled: true, source: format!("gallery:{}", self.id), icon: self.id.into(), added_at: now, ..Default::default() };
        if self.remote() { s.transport = "http".into(); s.url = self.url.into() } else {
            s.transport = "stdio".into(); s.command = self.command.into(); s.args = self.args.iter().map(|a| a.to_string()).collect();
            if !self.needs.is_empty() { s.args.push(arg.trim().to_string()) }
        }
        if let Some((k, pre, _)) = self.secret { let kv = Kv::sec(k, &super::model::vault_name(&id, k), pre); if self.remote() { s.headers.push(kv) } else { s.env.push(kv) } }
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn every_entry_makes_a_valid_server_or_says_why_not() {
        let mut ids = vec![];
        for e in ENTRIES {
            assert!(!ids.contains(&e.id), "{} twice", e.id); ids.push(e.id);
            assert!(e.home.starts_with("https://") && !e.about.is_empty() && e.about.len() <= 60, "{}", e.id);
            match e.server(&[], "/tmp/x", 1) {
                Some(s) => { assert_eq!(s.problem(), None, "{}", e.id); assert_eq!(s.source, format!("gallery:{}", e.id)); assert!(!e.signin) }
                None => assert!(e.signin, "{} adds nothing", e.id),
            }
        }
        assert!(get("filesystem").unwrap().server(&[], " ", 1).is_none(), "a folder is needed");
        assert_eq!(get("filesystem").unwrap().server(&[], "/w", 1).unwrap().args.last().unwrap(), "/w");
        let gh = get("github").unwrap().server(&["github".into()], "", 1).unwrap(); assert_eq!(gh.id, "github-2"); assert_eq!(gh.headers[0].v, "mcp-github-2-authorization"); assert_eq!(gh.headers[0].pre, "Bearer ");
        assert!(ENTRIES.iter().filter(|e| e.community).all(|e| e.by != "MCP reference"));
    }
}
