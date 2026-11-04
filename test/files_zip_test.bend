import Base
import ../src/core/fm.bend as FM
import ../src/core/auth.bend as AUTH
import ../src/core/http.bend as H

# Driven by test/tools/files_zip.mjs: the hub's zip writer
# (src/server/effects/zip.js) reads core/fm.bend's rules as data; the
# driver checks it decides exactly as Fm.zip.skip, Fm.zip.within,
# Fm.zip.store and Fm.zip.entry do over many generated paths, then makes
# real zips with the plan the hub makes (server.bend's FilesHttp.zip).

def rules() -> List<&2, String>:
  FM.Fm.zip.rules()

def stored() -> List<&2, String>:
  FM.Fm.zip.stored()

def swap() -> String:
  FM.Fm.zip.swap()

def note() -> String:
  FM.Fm.zip.note()

def note_head() -> String:
  FM.Fm.zip.note.head()

def why() -> List<&2, String>:
  FM.Fm.zip.why()

def skip(p: String) -> Bool:
  FM.Fm.zip.skip(p)

def within(p: String) -> Bool:
  FM.Fm.zip.within(p)

def store(name: String) -> Bool:
  FM.Fm.zip.store(name)

def entry(name: String) -> String:
  FM.Fm.zip.entry(name)

def top(title: String, dir: String) -> String:
  FM.Fm.zip.top(title, dir)

def files_safe(rel: String) -> Bool:
  FM.Fm.files.safe(rel)

def disposition(name: String) -> String:
  FM.Fm.disposition(name)


def allowed(token: String, credential: String, site: String) -> Bool:
  AUTH.Auth.allowed([H.Header{"authorization", "Bearer " ++ credential}, H.Header{"sec-fetch-site", site}], "/files/get?thread=t&path=a", token)
