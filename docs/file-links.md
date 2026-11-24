# Files for every client: links, downloads and shares

A path an agent writes is a file on the hub's machine. A client on another machine (a linked desktop, a phone, the web app) cannot open the path
itself, so the hub serves it. Hub side: `native/host/src/files/share.rs`.

## The access rule (`share::check`)
A path (absolute, `~/` or `file://`) is served when its real file (every link followed) is a regular file under one of these roots:
- a live project's folder,
- a live thread's worktree,
- the hub's own `<home>/attachments` and `<home>/far-files`,
- each line of the setting `files.share.roots`.

Neither `/` nor the user's whole home is ever a root. Each folder between the root and the file must be shown: not hidden (a dot name other
than `.backplane` and `.github`), not `secrets`, `vault`, `node_modules` or a `.backplane-*` home. The file's name must not be a secret
(`.env*`, `id_rsa*`, `*.pem`, `*.key`, `credentials*`, `secrets*`, `token`, `settings.local.json`, ...) or a dotfile (except `.backplane.json`
and `.gitignore`). Nothing under the hub's home is served except its two folders. A link from a project to a file outside it is the file
outside it, so it is refused. Refusals: 404 `no such file` or `not a file`, 403 for the rest (`that file is not in a project`, `that file is
not shared`).

Who may ask: the hub token or a live paired device's token (Bearer, cookie or `?token=`, as for every route), and on the web ingress a paired
browser's cookie. A device that has been revoked is refused on its next request. The hub never answers 304: a conditional or ranged
request is checked again like any other, so a revoked device or browser gets 401 even with a valid ETag.

A paired browser (`Who::Browser`) gets less: only files under projects, worktrees and `<home>/attachments` (not `far-files` and not
`files.share.roots`). With `thread=<id>` the file must also be inside that thread's folder.

## Routes (streamed, never held whole in memory)
- `GET|HEAD /file?path=<abs path>` returns the file.
- `GET|HEAD /file?thread=<link>~<id>&path=<abs or project-relative>` returns a file of a thread on another of the owner's machines. It is read
  there in 4 MiB pieces over the link (`file.piece`, a far RPC; the owning hub applies the same rule plus "inside that thread's folder") and
  streamed on.
- `GET|HEAD /files/get?thread=<id>&path=<rel>` is the Files tab's Download (and Mech's). It applies the Files tab's containment, and a `<link>~<id>`
  thread works as above.
- `GET|HEAD /s/<token>/<name>` is a share. On the hub's main listener (native clients, and other apps on the tailnet), the token is the only
  credential needed. On the web ingress a share answers only to a paired browser: an unpaired browser reaches nothing but pairing there.
  The token is 192 random bits, compared in constant time. It answers 404 once the share is unknown, expired or revoked. An address with 30
  unknown tokens in a minute gets 429 until the minute ends. A share names one file and can never list or reach another.

Every answer has `Accept-Ranges: bytes`, a strong `ETag` (size and mtime), `Content-Length` and `Content-Disposition: attachment`.
`inline=1` makes only PNG, JPEG, GIF, WebP, BMP, PDF and plain text inline. HTML and SVG are never served as such: they go out as
`application/octet-stream` attachments. Every answer also carries `X-Content-Type-Options: nosniff`,
`Content-Security-Policy: default-src 'none'; …; sandbox`, `Cross-Origin-Resource-Policy: same-origin` and `Referrer-Policy: no-referrer`.
`Cache-Control` is `private, no-cache` for native clients and `private, no-store` for browsers and shares. A request may carry one `Range: bytes=a-b`
and `If-Range: <etag>`. When the tag still matches, the answer is a 206 of that range. When the file has changed since, the answer is a 200
with the whole file.

**Resuming downloads.** A client writes `<name>.part` and keeps the ETag. After an error it asks again with `Range: bytes=<have>-` and
`If-Range: <etag>`. A 206 means append to the `.part` file. A 200 means start over. When the length is complete, it renames the `.part` file
to the name. A body that ends short (the file shrank, or a far link dropped) is resumed the same way.

## Message links (data/1 feature `fileLinks`, and the desktop's thread rows)
An assistant entry carries `files: [{path, name, size, open}]`. These are the paths its text names that the rule allows, in the message's own
spelling, at most 32. `open` is `view` when Backplane's viewers draw the file (board, schematic, 3D, image, PDF, text note). Otherwise it is
`download`. A client draws each `path` in the text as a link:
- **Local client (same machine as the hub):** as before. The desktop's `link-file` opens it in the viewer, or in the system's app for files
  the viewers do not draw.
- **Remote client:** `view` opens it in Backplane's viewer as before (the viewers already read through the hub). `download` (and a Download item
  on every file link's menu) fetches `/file?path=` with the resume rule above, saves it to the device's Downloads folder or the share sheet,
  and then offers to open it.
For a thread of another machine (`<link>~<id>`), the request adds `thread=<that id>`.

## Shares (MCP tool `file_share {path, expires?, tailnet?}`)
A share is for opening a file outside Backplane's clients (another app, a browser on any device of the tailnet). It returns `{id, link, name,
size, expires}`. `link` is `/s/<token>/<name>`: an agent writes it in a message and every client opens it against its hub. With
`tailnet: true` the answer also has `url`, the same path on the hub's own tailnet address: its existing `tailscale serve` https address when
the person set one up, otherwise `http://<MagicDNS name>:<port>` when the hub listens on the tailnet. When it has neither, the answer gets
`tailnetError` instead. The hub makes no `tailscale serve` mapping for a share. A share is not a second ingress, only a path on the main
listener. `expires` is seconds or `30m`/`12h`/`7d`, default 1 day, at most 30 days. Shares are kept in `<home>/shares-v1.json` (0600). The file
is checked again on every request, so a share of a file that has since become a secret or left its project stops working.

**Settings.** Settings → Devices lists each live share (`Shared file · <name>`, its path and expiry, and its tailnet address) with **Revoke**
(`device-share-revoke`). The phones' `devices.page` draws the same rows and its Revoke goes through `devices.page.act`. The desktop's
protocol (`/ws`, CBOR) also answers `shares.list` → `{shares}` and `shares.revoke {id}` → `{shares}`; data/1 has no such methods.

## The Files and Mech tabs
- Files rows have `down` (a `/files/get` URL) and `downName`. A row on another machine's thread now has one too (a folder's zip does not). When
  the project has `mfg/`, the page carries `mfg: "mfg"`: a client draws a **Manufacturing** button that sends `fm-cd mfg` (Gerbers, BOM, CPL,
  each with its Download).
- Mech: each part chip has `down` and `downName`, and so does the part on show (`mech.down`). A client draws a Download button for it.

## Not in the view deltas
Downloads are plain HTTP on the connection the client already has to its hub (the hub's address, or the web ingress). They are not part of
the view or data/1 deltas and do not count against their byte budgets.
