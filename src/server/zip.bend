import Base

# The Files tab's downloads (server.bend's FilesHttp; core/fm.bend decides
# every policy: the rules, what is stored, names, the note's words).
# effects/zip.js does the IO.

# rel (a path from root) with every link resolved, while it stays in root:
# [the root's real path, the path from it ("/" between names, "" for the
# root itself), "d" a folder | "f" a file | "o" anything else]; [] when it
# is gone or leaves root
def Zip.inside(root: String, rel: String, home: String) -> IO(List<&2, String>):
  import "./effects/zip.js"

# A Files-tab listing, opened through the same descriptor-bound containment
# checks as downloads. [entries] on success (including an empty directory),
# [] on refusal. Symlinks and non-regular entries are never exposed.
def Zip.list(root: String, rel: String, home: String) -> IO(List<&2, String>):
  import "./effects/zip.js"

# A zip of the folder rel of root, both as Zip.inside answered them:
# top, the folder every entry is in; disposition, the attachment's header;
# rules, what is left out (Fm.zip.rules, by each entry's path from root);
# stored, the name ends kept as they are (Fm.zip.stored); swap, the
# characters an entry's name holds as "_" (Fm.zip.swap); note, head and
# why, the file at the top that says what could not be read
# (Fm.zip.note, Fm.zip.note.head, Fm.zip.why).
type ZipPlan is Data:
  ZipPlan{root: String, rel: String, home: String, maxBytes: U32, maxEntries: U32, top: String, disposition: String, rules: List<&2, String>, stored: List<&2, String>, swap: String, note: String, head: String, why: List<&2, String>}

# The zip streamed as a chunked HTTP answer: each file a piece at a time,
# deflated (level 1) or stored, with data descriptors, UTF-8 names, ZIP64
# past 4 GB or 65535 entries; never a whole file or the archive in memory,
# and the loop yields after every piece. Links are never followed; a
# folder that turned into one while the zip was made is left out.
def Zip.send(sock: Socket, plan: ZipPlan) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/zip.js"

# A file opened within the same confined root as a zip, streamed with ranges.
def Zip.file(sock: Socket, root: String, rel: String, home: String, disposition: String, range: String, rules: List<&2, String>) -> IO(Socket & Result<&1, &1, U32 & String, Unit>):
  import "./effects/zip.js"
