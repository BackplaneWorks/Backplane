# Browsers

The agent's browser is Chrome, driven by `backplane-browser` (`tools/browser/`,
CDP through playwright-core). Each thread or bot has its own page in one
persistent profile; its JPEG frames reach every client, the native window
included. While the agent drives, the person watches; Take over gives them the
page (the hub's drive state, `docs/teach.md`) and Hand back returns it.

The desktop window's in-app browser is Electron's own Chromium, in a
`WebContentsView`.

There is no embedded browser in the native window and no C or C++ in the
repository.
