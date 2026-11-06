# Images in agent messages

An assistant reply can show an absolute PNG, JPEG, GIF, WebP or BMP path.
The web and phone projections use `/message-image?entry=…&path=…`. Each GET
requires a current pairing credential (browser HttpOnly cookie, native Bearer
header or native token query), including on loopback. Device revocation takes
effect on the next request. Image responses are private and must revalidate.

The entry ID is not a credential. The hub loads the saved assistant message
from its derived store and uses the same path parser as the clients to check
an exact reference. User messages, tool output, deleted threads, unreferenced
paths, traversal and symbolic links cannot authorize this route. The existing
Files preview route remains separate.

After committing an assistant message, the hub retains valid images under
`<hub home>/message-images/`, in a private directory. Content hashes deduplicate
bytes; an entry-and-path hash identifies each retained reference. Older messages
are retained when first requested, while their source still exists. A removed
worktree therefore does not remove an image already retained. Existing missing
source files cannot be recovered automatically.

Each image is limited to 24 MiB encoded and 32 megapixels. The hub verifies
raster signatures and dimensions before retaining or serving it; SVG and HTML
are rejected. Retained images are immutable snapshots rather than live file
previews. Web thumbnails load lazily with fixed geometry. Failed images show
“Image unavailable”; native thumbnails have a text failure state too.

Run `node --test test/tools/message_images_test.cjs desktop/test/image-actions.test.cjs`
for boundary tests and `test/tools/message_images_verify.sh` for checks,
proof verdict, and candidate builds. `test/tools/message_images_browser.mjs`
accepts its disposable device credential on stdin and tests a 390px mobile web
conversation from a second machine; the caller must revoke the TEST device in
its cleanup path. Never reuse a person's real device for acceptance.
