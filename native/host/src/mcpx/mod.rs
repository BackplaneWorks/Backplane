//! MCP servers for every agent (docs/MCP.md): the list (model.rs), what is pasted or found in other tools (parse.rs, toml.rs, import.rs),
//! each provider's config (gen.rs).
pub mod gen;
pub mod import;
pub mod model;
pub mod parse;
pub mod toml;
