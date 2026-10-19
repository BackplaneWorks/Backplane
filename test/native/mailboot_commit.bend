import Base
import ../mailboot_test.bend as T
import ../mailstore_test.bend as TMP
import ../check.bend as C
import ../../src/server/server.bend as S
import ../../src/server/host.bend as H
import ../../src/server/botnet.bend as BN
import ../../src/core/store.bend as ST
import ../../src/core/model.bend as M

def reset_done(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, got) = r
  do IO<Unit>:
    IO.try(Unit, IO.pure(Result<&1, &1, U32 & String, Unit>, got))
    File.close(f)

def reset(home: String) -> IO(Unit):
  do IO<Unit>:
    f : File <- IO.try(File, File.open(S.Store.path(home), "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write(f, ST.Store.text(T.disk()))
    reset_done(r)

def checked(r: S.Hub & S.Wrote, +want: String, +want_n: Nat) -> IO(Unit):
  (h, w) = r
  match h w:
    case S.Hub{st, f, +ls, _, _, inbox, +cfg, _} S.Wrote{fresh, ok, _}:
      do IO<Unit>:
        C.Check.str("actual write success flag " ++ want, Bool.show(ok), want)
        T.count("actual write updates count only on success " ++ want, cfg, want_n)
        C.Check.str("actual Hub wire equals cached count " ++ want, Nat.show(List.length(&2, String, ls)), Nat.show(S.LogCount.n(cfg)))
        C.Check.str("actual fresh lines " ++ want, Nat.show(List.length(&2, String, fresh)), Nat.show(Nat.sub(want_n, 3n)))
        File.close(f)
        Chan.close(S.Msg, inbox)

def run(+home: String, mode: String, cs: List<&2, M.Change>, want: String, want_n: Nat) -> IO(Unit):
  do IO<Unit>:
    reset(home)
    f : File <- IO.try(File, File.open(S.Store.path(home), mode))
    inbox : Chan(S.Msg) <- Chan.new(S.Msg, 8)
    +ld : ST.Loaded = T.loaded()
    +ls : List<&2, String> = ST.Loaded.wire(ld)
    r : S.Hub & S.Wrote <- S.Hub.commit.w(cs,
      S.Hub{ST.Loaded.st(ld), f, ls, Nil{}, Nil{}, inbox, S.LogCount.init(T.cfg(home, BN.Bn.empty()), ls), ""})
    checked(r, want, want_n)

def main() -> IO(Unit):
  do IO<Unit>:
    # host.c's unconditional join helpers need this codec dependency, as in
    # native/mailstore_test.bend. This creates no connection or runtime hub.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    C.Check.str("host codec dependency", Nat.show(List.length(&2, U32, frame)), "3")
    T.main()
    epoch : U32 <- H.Clock.epoch()
    +home : String <- TMP.home.at("/tmp/bp-mailboot-test-" ++ U32.show(epoch))
    # Both descriptors name our synthetic file. The read-only descriptor
    # produces a real failed File.write and exercises commit rollback.
    run(home, "a", [M.SettingSet{"test.commit", "yes", 4n}], "True", 4n)
    run(home, "r", [M.SettingSet{"test.commit", "yes", 4n}], "False", 3n)
    run(home, "a", [M.DeviceDropped{"synthetic-private-token", 4n}, M.SettingSet{"test.batch", "yes", 5n}], "True", 5n)
    run(home, "a", Nil{}, "True", 3n)
