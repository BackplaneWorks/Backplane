//! MCP servers for every agent (docs/MCP.md): the list (model.rs), what is pasted or found in other tools (parse.rs, toml.rs, import.rs),
//! each provider's config (gen.rs), the wrapper agents start a stdio server through (run.rs), the live check (health.rs), the gallery,
//! Settings > MCP (page.rs, act.rs), the agents' mcp_ tools (tools.rs) and the service that holds it all (svc.rs).
pub mod act;
pub mod gallery;
pub mod gen;
pub mod health;
pub mod import;
pub mod model;
pub mod page;
pub mod parse;
pub mod run;
pub mod svc;
pub mod toml;
pub mod tools;
