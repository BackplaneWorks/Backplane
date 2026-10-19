<p align="right">
  <a href="README.jp.md"><img src="docs/media/readme/language-ja.svg" alt="日本語" width="92" height="28"></a>
</p>

1. Backplane is a TOTALLY open-source workspace for agentic hardware development.
2. Backplane is a [KiCad fork](https://github.com/i2cjak/Backplane_KiCad).
3. Backplane is created + maintained by real electrical engineers.

<p align="center">
  <img src="docs/media/readme/royalblue-viewers-200.png" alt="RoyalBlue54L in Backplane’s 3D, PCB, and schematic viewers">
</p>

```sh
curl -fsSL https://github.com/i2cjak/backplane-bend/releases/latest/download/install.sh | sh
backplane
```

<p align="center">
  <img src="docs/media/readme/agent-setup-200.png" alt="Backplane’s provider, model, and reasoning effort picker">
</p>

The real VALUE of Backplane is in its KiCad + FreeCAD viewers. See EVERY step the agent takes, click on specific components and reference them in the workbar. Backplane ships with a FULL suite of CAD-specific skills and a KiCad fork that adds additional instrumentation for LLM control (IPC schematic API, for instance).

<p align="center">
  <img src="docs/media/readme/cad/pad-workbar-detail-200.png" alt="Select a RoyalBlue54L pad and mention its details in the workbar">
</p>

<p align="center">
  <img src="docs/media/readme/cad/cm5-mech-200.png" alt="A case assembly in Backplane’s mechanical viewer alongside the agent’s work">
</p>

<p align="center">
  <img src="docs/media/readme/cad/kicad-freecad-setup-200.png" alt="KiCad IPC and FreeCAD integration settings">
</p>

Backplane features some nice harness orchestration primitives. We have projects that contain threads (wow, a truly novel idea), bots, and an "uberbot" that coordinates bots, projects, and threads. Any harness can create subagents with any other harness, and all harnesses can send messages between threads.

<p align="center">
  <img src="docs/media/readme/orchestration/overview.png" alt="Uberbot overview with projects, threads, and persistent bots">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/delegation.png" alt="A thread coordinating Claude and Grok subagents">
</p>

We also ship persistent agents (catbots) to handle specific tasks. They have webhooks, etc., whatever you expect BUT also ship with a nice mailbox system for communication and a "space" in which the bot can create a persistent webapp.

<p align="center">
  <img src="docs/media/readme/orchestration/webhooks.png" alt="A catbot’s Webhooks tab">
</p>

<p align="center">
  <img src="docs/media/readme/mailroom-200.png" alt="A catbot’s Mailroom with conversations and an open message">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/space.png" alt="A persistent control desk in a catbot’s Space">
</p>

<p align="center">
  <img src="docs/media/readme/orchestration/space-webapp.png" alt="A webapp served from a catbot’s persistent Space" width="480">
</p>

## Other features

- Clickable links between threads and bots and projects.

  <p align="center">
    <img src="docs/media/readme/features/link-cards-2x.png" alt="Rendered thread, bot, and project link chips">
  </p>

- Links to other Backplane items with `>`, `#`, `%`, `@`.

  <p align="center">
    <img src="docs/media/readme/reference-menus-200.png" alt="Thread, room, project, and bot reference menus">
  </p>

- Inline terminals for TUIs.

  <p align="center">
    <img src="docs/media/readme/features/inline-terminal-2x.png" alt="An actual vi session in an inline terminal">
  </p>

- `/btw`.

  <p align="center">
    <img src="docs/media/readme/features/btw-autocomplete-2x.png" alt="The btw side-question command in the composer">
  </p>

Backplane works with WHATEVER agent you choose to bring. I'm not going to stop you from using Grok, Codex, Claude, Antigravity, Muse, whatever. We even have OpenCode support!

## License

MIT. See [LICENSE](LICENSE). The [KiCad fork](https://github.com/i2cjak/Backplane_KiCad) retains KiCad's licensing. The STEP converter includes OpenCascade and `occt-import-js` (LGPL-2.1); see [third-party licensing](tools/step2glb/README.md#license).

<p align="center">
  <img src="docs/media/readme/cad/cats-footer-200.png" alt="Three Backplane cats" width="216" height="72">
</p>
