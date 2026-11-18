# Provider marks

One vector mark per built-in agent, drawn tinted by the client (`native/gpui/src/art.rs` `mark_path`). Any other provider gets a coloured square with an initial.
Checked 2026-11-18 against each upstream; where the source says "official artwork", it is the artwork the product's own site or repository publishes.

| File | Agent | Source | Licence / terms |
|---|---|---|---|
| claude.svg | Claude Code | simple-icons `icons/claude.svg` 16.36.0 (https://github.com/simple-icons/simple-icons), Anthropic's mark | CC0 1.0 (simple-icons' data); trademark of Anthropic |
| opencode.svg | OpenCode | simple-icons `icons/opencode.svg` 16.36.0, from `packages/identity/mark.svg` in https://github.com/anomalyco/opencode | CC0 1.0 (the data); trademark of the OpenCode project |
| pi.svg | Pi | simple-icons `icons/pi.svg` 16.38.0, its source is https://pi.dev/favicon.svg (the product's own favicon) | CC0 1.0 (the data); the Pi project's mark |
| muse.svg | Muse Code | simple-icons `icons/metaai.svg` 16.38.0, from Meta AI's orbit logo (https://meta.ai/images/meta-ai-orbit-logo-gradient-3d_light.svg). Muse Code is Meta's (https://dev.meta.ai/docs/muse-code); it publishes no mark of its own, so the Meta AI one stands for it | CC0 1.0 (the data); trademark of Meta Platforms |
| grok.svg | Grok | The glyph (two paths) of the app icon https://grok.com/images/favicon.svg, cropped and rescaled to 24x24 without its tile, blur and shadow. Official artwork | No open licence found; xAI's trademark. Shown to identify the agent |
| antigravity.svg | Antigravity | The arch of the favicon https://antigravity.google/favicon.svg, rescaled to 24x24 with its gradient and glow dropped (one tint). Official artwork | No open licence found; Google's trademark. Shown to identify the agent |
| omp.svg | Oh My Pi | `assets/icon.svg` of https://github.com/can1357/oh-my-pi (a pi sign with a plug), redrawn as one path in one tint: the plug is a separate block under the right leg, where the original overlaps it in another colour | MIT (the repository, Copyright Mario Zechner and Can Bölük) |
| codex.svg | Codex | **Drawn for Backplane**: a terminal prompt (a chevron and an underscore) | Backplane's own. Not OpenAI's mark |

## What is not an official mark

- **Codex:** the `openai/codex` repository (Apache-2.0) ships no product mark: its only SVGs are skill icons (`openai-small.svg` is the OpenAI Docs skill's book), and OpenAI's blossom is not in simple-icons any more. Rather than borrow another OpenAI product's art, the row uses the prompt glyph above, which says "command-line agent". Swap in OpenAI's mark if OpenAI's brand terms are accepted for it.
- **Muse Code:** the Meta AI orbit, as above. There is no Muse Code mark to use.

## Terms

CC0 covers simple-icons' files, not the trademarks. Grok's and Antigravity's artwork has no licence the project states: it is shown only to say which agent a row is (nominative use), never as Backplane's own branding. If an owner asks, or the person decides against it, delete the file: `mark_path` and the picker fall back to the square.
The marks are tinted a single colour by the client, so none keeps its gradient, glow or tile.
