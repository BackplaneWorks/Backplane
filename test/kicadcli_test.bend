import Base
import ./check.bend as C
import ../src/core/kicadcli.bend as KC
import ../src/core/json.bend as J
import ../src/core/kihealth.bend as KH

def identity_doc() -> J.Json:
  J.Json.obj([J.KV{"sourceRepository", J.Str{"https://github.com/i2cjak/Backplane_KiCad"}}, J.KV{"sourceCommit", J.Str{"5dbc22d292704d02b609d5b7a0edc971f1b7f08b"}}, J.KV{"version", J.Str{"10.99.0-bp.4"}}, J.KV{"kicadVersion", J.Str{"10.99.0"}}, J.KV{"apiVersion", J.Num{"2"}}, J.KV{"platform", J.Str{"linux-x86_64"}}, J.KV{"apiDescriptor", J.Str{"share/backplane/api.desc"}}, J.KV{"capabilities", J.Json.arr([J.Str{"api-server"}, J.Str{"headless-pcb"}, J.Str{"headless-sch"}])}])

def identity(describe: J.Json) -> Bool:
  KC.Cli.identity(identity_doc(), describe, "10.99.0\n", "10.99.0-bp.4", "linux-x86_64")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Linux bp.4 published", Bool.show(KC.Cli.published("Linux x86_64\n")), "True")
    C.Check.str("Mac layout does not promise a published asset", Bool.show(KC.Cli.published("Darwin arm64")), "False")
    C.Check.str("unsupported architecture", Bool.show(KC.Cli.published("Linux aarch64")), "False")
    C.Check.str("legacy Linux compatible", Bool.show(KC.Cli.compatible("Linux x86_64", "")), "True")
    C.Check.str("legacy Linux incompatible on Mac", Bool.show(KC.Cli.compatible("Darwin arm64", "")), "False")
    C.Check.str("explicit platform incompatible", Bool.show(KC.Cli.compatible("Linux x86_64", "macos-arm64")), "False")
    C.Check.str("Mac manifest compatible", Bool.show(KC.Cli.compatible("Darwin arm64", "macos-arm64")), "True")
    C.Check.str("unsupported manifest incompatible", Bool.show(KC.Cli.compatible("Linux aarch64", "")), "False")
    C.Check.str("Mac never scans legacy installs", Bool.show(KC.Cli.installed("10.99.0-bp.4", "Darwin arm64")), "False")
    C.Check.str("Linux never scans Mac installs", Bool.show(KC.Cli.installed("10.99.0-bp.4-macos-arm64", "Linux x86_64")), "False")
    C.Check.str("Mac isolated install", KC.Cli.dir("/tmp/data", "10.99.0-bp.4", "macos-arm64"), "/tmp/data/kicad/10.99.0-bp.4-macos-arm64")
    C.Check.str("real release separates CLI core from fork version", Bool.show(identity(identity_doc())), "True")
    C.Check.str("empty identity rejected", Bool.show(identity(J.Json.obj(Nil{}))), "False")
    C.Check.str("identity array rejected", Bool.show(identity(J.Json.arr(Nil{}))), "False")
    C.Check.str("wrong fork version rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "version", J.Str{"10.99.0-bp.3"}))), "False")
    C.Check.str("wrong upstream CLI version rejected", Bool.show(KC.Cli.identity(identity_doc(), identity_doc(), "9.0.0-bp.4", "10.99.0-bp.4", "linux-x86_64")), "False")
    C.Check.str("wrong API rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "apiVersion", J.Num{"1"}))), "False")
    C.Check.str("fractional API rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "apiVersion", J.Num{"2.5"}))), "False")
    C.Check.str("string API rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "apiVersion", J.Str{"2"}))), "False")
    C.Check.str("wrong platform rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "platform", J.Str{"macos-arm64"}))), "False")
    C.Check.str("wrong source rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "sourceCommit", J.Str{"other"}))), "False")
    C.Check.str("missing capabilities rejected", Bool.show(identity(KH.KH.put.obj(identity_doc(), "capabilities", J.Json.arr([J.Str{"api-server"}])))), "False")
    C.Check.str("malformed manifest rejected", Bool.show(KC.Cli.identity(J.Null{}, identity_doc(), "10.99.0", "10.99.0-bp.4", "linux-x86_64")), "False")
