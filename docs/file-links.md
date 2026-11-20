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
browser's cookie. A device that has been revoked is refused on its next request.

## Routes (streamed, never held whole in memory)
- `GET|HEAD /file?path=<abs path>` returns the file.
- `GET|HEAD /file?thread=<link>~<id>&path=<abs or project-relative>` returns a file of a thread on another of the owner's machines. It is read
  there in 4 MiB pieces over the link (`file.piece`, a far RPC; the owning hub applies the same rule plus "inside that thread's folder") and
  streamed on.
- `GET|HEAD /files/get?thread=<id>&path=<rel>` is the Files tab's Download (and Mech's). It applies the Files tab's containment, and a `<link>~<id>`
  thread works as above.
- `GET|HEAD /s/<token>/<name>` is a share. The token is the only credential needed. It answers 404 once the share expires or is revoked.

Every answer has `Accept-Ranges: bytes`, a strong `ETag` (size and mtime), `Content-Length` and `Content-Disposition: attachment` (`inline=1`
for images, PDFs and text). It also has `X-Content-Type-Options: nosniff` and a sandboxing CSP. A request may carry one `Range: bytes=a-b`
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
`tailnet: true` the answer also has `url`, an https address on the person's tailnet. It uses the hub's own `tailscale serve` address when
there is one. Otherwise the hub serves an https port in 8443–8499 for shares. That port is taken down by the minute sweep once no live share
uses it. `expires` is seconds or `30m`/`12h`/`7d`, default 1 day, at most 30 days. Shares are kept in `<home>/shares-v1.json` (0600). The file
is checked again on every request, so a share of a file that has since become a secret or left its project stops working.

**Settings.** Settings → Devices lists each live share (`Shared file · <name>`, its path and expiry, and its tailnet address) with **Revoke**
(`device-share-revoke`). The phones' `devices.page` draws the same rows. The data RPCs are `shares.list` → `{shares}` and `shares.revoke
{id}` → `{shares}`.

## The Files and Mech tabs
- Files rows have `down` (a `/files/get` URL) and `downName`. A row on another machine's thread now has one too (a folder's zip does not). When
  the project has `mfg/`, the page carries `mfg: "mfg"`: a client draws a **Manufacturing** button that sends `fm-cd mfg` (Gerbers, BOM, CPL,
  each with its Download).
- Mech: each part chip has `down` and `downName`, and so does the part on show (`mech.down`). A client draws a Download button for it.

## Not in the view deltas
Downloads are plain HTTP on the connection the client already has to its hub (the hub's address, or the web ingress). They are not part of
the view or data/1 deltas and do not count against their byte budgets.
