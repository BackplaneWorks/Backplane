import Base
import ./check.bend as C
import ../src/core/issue-report.bend as R
import ../src/core/json.bend as J
import ../src/core/utf8.bend as UTF

def report() -> String:
  R.Issue.report("  Repro 日本語 🐛\r\n<literal>\n", "linux", "v1.2.3-nightly.20261102+local", "", "")

def body() -> String:
  "  Repro 日本語 🐛\r\n<literal>\n\n\n---\nOS: Linux\nApp version: 1.2.3\n"

def preview() -> J.Json:
  R.Issue.preview("issue & #? 日本語", "iOS", "2.3.4", "", "")

def repeat(n: Nat, +s: String, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: repeat(k, s, s ++ acc)

def long_text() -> String:
  repeat(10000n, "x", "")

def prompt() -> String:
  R.Issue.prompt("  I saw a blank page.\r\n日本語 <literal>  ", "linux", "v1.2.3+private.owner", "The page stayed blank for 30 seconds after opening Settings.", "{\"count\":2}")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("new report defaults public", R.Issue.target("", False{}), "issue")
    C.Check.str("legacy internal draft kept", R.Issue.target("", True{}), "internal")
    C.Check.str("explicit internal new report", R.Issue.target("internal", False{}), "internal")
    C.Check.str("explicit issue overrides legacy", R.Issue.target("issue", True{}), "issue")
    C.Check.str("unrecognized new target defaults public", R.Issue.target("/home/owner/project", False{}), "issue")
    C.Check.str("unrecognized legacy target stays internal", R.Issue.target("other", True{}), "internal")
    C.Check.str("exact target tokens only", R.Issue.target("INTERNAL", False{}), "issue")
    C.Check.str("user text byte for byte", report(), body())
    C.Check.str("manual details and selected JSON verbatim",
      R.Issue.report("words", "Android", "1.2.3", "  build details\r\n`literal`  ", "{\n  \"ok\": true\n}"),
      "words\n\n---\nOS: Android\nApp version: 1.2.3\n\n\nDetails\n  build details\r\n`literal`  \n\nDiagnostics (JSON)\n{\n  \"ok\": true\n}")
    C.Check.str("OS windows alias", R.Issue.os("win32"), "Windows")
    C.Check.str("OS mac alias", R.Issue.os("Darwin"), "macOS")
    C.Check.str("OS iPad family", R.Issue.os("iPadOS"), "iOS")
    C.Check.str("OS never echoes machine descriptions", R.Issue.os("Linux host.owner /private/path"), "Unknown")
    C.Check.str("OS never accepts paths", R.Issue.os("/home/owner/project"), "Unknown")
    C.Check.str("OS empty fallback", R.Issue.os(""), "Unknown")
    C.Check.str("version strips build metadata", R.Issue.version("v1.2.3+owner.machine"), "1.2.3")
    C.Check.str("version rejects local path", R.Issue.version("/home/owner/build/1.2.3"), "unknown")
    C.Check.str("version rejects injected metadata", R.Issue.version("1.2.3\nOwner: private"), "unknown")
    C.Check.str("version rejects suffix path", R.Issue.version("1.2.3-dev./home/owner"), "unknown")
    C.Check.str("version rejects missing number", R.Issue.version("1..3"), "unknown")
    C.Check.str("version rejects extra number", R.Issue.version("1.2.3.4"), "unknown")
    C.Check.str("version rejects unbounded number", R.Issue.version("1234567.2.3"), "unknown")
    C.Check.str("version rejects overlong input", R.Issue.version(repeat(100n, "a", "1.2.3+")), "unknown")
    C.Check.str("empty fields have safe fallback", R.Issue.report("", "", "", "", ""), "\n\n---\nOS: Unknown\nApp version: unknown\n")
    C.Check.str("no inferred redaction of manual words", R.Issue.report("/home/person/project", "unknown", "unknown", "", ""), "/home/person/project\n\n---\nOS: Unknown\nApp version: unknown\n")
    C.Check.str("fixed issue repository", R.Issue.new_url(), "https://github.com/BackplaneWorks/Backplane/issues/new")
    C.Check.str("query UTF-8 and reserved characters", R.Issue.url.encode(UTF.Utf8.encode("a b&#?%日本語🐛"), 200n, "", True{}), "a%20b%26%23%3F%25%E6%97%A5%E6%9C%AC%E8%AA%9E%F0%9F%90%9B")
    C.Check.str("preview preserves complete body", J.Json.get_str(preview(), "body", ""), R.Issue.report("issue & #? 日本語", "iOS", "2.3.4", "", ""))
    C.Check.str("preview encodes exact body", J.Json.get_str(preview(), "url", ""), R.Issue.url.prefix() ++ "issue%20%26%20%23%3F%20%E6%97%A5%E6%9C%AC%E8%AA%9E%0A%0A---%0AOS%3A%20iOS%0AApp%20version%3A%202.3.4%0A")
    C.Check.str("encoder exact ASCII budget", R.Issue.url.encode(UTF.Utf8.encode("abc"), 3n, "", True{}), "abc")
    C.Check.str("encoder rejects overflow by one", R.Issue.url.encode(UTF.Utf8.encode("abcd"), 3n, "", True{}), "")
    C.Check.str("encoder exact escaped budget", R.Issue.url.encode(UTF.Utf8.encode("?"), 3n, "", True{}), "%3F")
    C.Check.str("encoder rejects final escape overflow", R.Issue.url.encode(UTF.Utf8.encode("?"), 2n, "", True{}), "")
    C.Check.str("URL exactly 8000 ASCII bytes", Bool.show(String.eq(R.Issue.url.body(repeat(Nat.sub(8000n, String.length(R.Issue.url.prefix())), "x", "")), R.Issue.url.prefix() ++ repeat(Nat.sub(8000n, String.length(R.Issue.url.prefix())), "x", ""))), "True")
    C.Check.str("URL over 8000 bytes returns no link", R.Issue.url.body(repeat(Nat.sub(8001n, String.length(R.Issue.url.prefix())), "x", "")), "")
    C.Check.str("oversize preview retains original body", J.Json.get_str(R.Issue.preview.body(long_text()), "body", ""), long_text())
    C.Check.str("oversize preview has no truncated URL", J.Json.get_str(R.Issue.preview.body(long_text()), "url", "fallback"), "")
    C.Check.str("report template version", R.Issue.template.version(), "1")
    C.Check.str("template asks reproduction steps", Bool.show(String.contains(R.Issue.template(), "## Steps to reproduce\n\n1. Not provided.")), "True")
    C.Check.str("template separates expected and actual", Bool.show(Bool.and(String.contains(R.Issue.template(), "## Expected behavior"), String.contains(R.Issue.template(), "## Actual behavior"))), "True")
    C.Check.str("template asks image, recording or thorough description", Bool.show(String.contains(R.Issue.template(), "Add an image, a screen recording or a thorough description of the behavior.")), "True")
    C.Check.str("prompt actually embeds template", Bool.show(String.contains(prompt(), R.Issue.template())), "True")
    C.Check.str("prompt actually embeds exact skill", Bool.show(String.contains(prompt(), R.Issue.unslop())), "True")
    C.Check.str("prompt includes supplied evidence", Bool.show(String.contains(prompt(), "Supplied evidence\nThe page stayed blank for 30 seconds after opening Settings.")), "True")
    C.Check.str("prompt includes selected JSON only", Bool.show(String.contains(prompt(), "Supplied optional diagnostics JSON\n{\"count\":2}")), "True")
    C.Check.str("prompt normalizes environment", Bool.show(String.contains(prompt(), "Supplied OS family\nLinux\nSupplied app version\n1.2.3\n")), "True")
    C.Check.str("prompt strips local version suffix", Bool.show(String.contains(prompt(), "private.owner")), "False")
    C.Check.str("prompt preserves original description at end", Bool.show(String.ends_with(prompt(), "Original description, verbatim, until end of input\n  I saw a blank page.\r\n日本語 <literal>  ")), "True")
    C.Check.str("prompt forbids fact invention", Bool.show(String.contains(R.Issue.prompt.rules(), "Do not invent facts, steps, measurements")), "True")
    C.Check.str("prompt forbids ambient context", Bool.show(String.contains(R.Issue.prompt.rules(), "Do not inspect other files, paths, projects, threads, account profiles, GitHub authentication, logs or other context.")), "True")
    C.Check.str("prompt has simplified English advice without compliance claim", Bool.show(String.contains(R.Issue.prompt.rules(), "Do not claim ASD-STE100 compliance, certification, or dictionary validation.")), "True")
    C.Check.str("prompt retains exact manual words instruction", Bool.show(String.contains(R.Issue.prompt.rules(), "Preserve the original description verbatim")), "True")
    C.Check.str("prompt avoids duplicated environment footer", Bool.show(String.contains(R.Issue.prompt.rules(), "Do not append another environment footer.")), "True")
    C.Check.str("missing evidence stays missing in supplied fields", Bool.show(String.contains(R.Issue.prompt("The page is blank.", "linux", "1.2.3", "", ""), "Supplied evidence\n\nSupplied optional diagnostics JSON\n\nOriginal description")), "True")
    C.Check.str("missing evidence retains explicit template request", Bool.show(String.contains(R.Issue.prompt("The page is blank.", "linux", "1.2.3", "", ""), "## Evidence\n\nNot provided. Add an image, a screen recording or a thorough description of the behavior.")), "True")
    C.Check.str("missing evidence prompt cannot claim it viewed an image", Bool.show(String.contains(R.Issue.prompt("The page is blank.", "linux", "1.2.3", "", ""), "Do not say you viewed an image when given only a reference.")), "True")
    C.Check.str("unknown installation method remains unfilled", Bool.show(String.contains(R.Issue.prompt("The page is blank.", "linux", "1.2.3", "", ""), "Installation method: Not provided.")), "True")
    C.Check.str("prompt permits only explicitly attached image inspection", Bool.show(String.contains(R.Issue.prompt.rules(), "The only allowed tool is the existing attachment viewer, only for an image explicitly attached by the person for this report.")), "True")
    return Unit{}
