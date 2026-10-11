# Checks for the flake. The hub has no --version flag: an unknown argument
# is ignored and a hub starts. `version` is the equivalent, GET /hello.
{
  pkgs,
  lib,
  nixpkgs,
  module,
  package,
  sources,
}: let
  version = sources.version;
  revision = sources.kistackRevision;
  fixture = pkgs.runCommand "kistack-fixture" {} ''
    mkdir -p "$out/skills/pcb" "$out/.claude-plugin"
    printf '%s\n' '# pcb' > "$out/skills/pcb/SKILL.md"
    printf '%s\n' '{"name":"kistack","description":"fixture"}' > "$out/.claude-plugin/plugin.json"
  '';
  # recursiveUpdate: a plain // replaces services.backplane and drops enable.
  minimal = extra:
    lib.recursiveUpdate
    {
      system.stateVersion = "26.05";
      boot.loader.grub.enable = false;
      fileSystems."/" = {
        device = "nodev";
        fsType = "tmpfs";
      };
      services.backplane.enable = true;
      services.backplane.package = package;
    }
    extra;
  eval = extra:
    nixpkgs.lib.nixosSystem {
      system = pkgs.stdenv.hostPlatform.system;
      modules = [
        module
        (minimal extra)
      ];
    };
  good = eval {};
  badFile = eval {services.backplane.environmentFile = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-secret";};
  badCred = eval {services.backplane.credentials.TOKEN = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-token";};
  withTailscale = eval {services.backplane.tailscale.enable = true;};
  failed = system: lib.filter (a: !a.assertion) system.config.assertions;
  mentions = system: needle:
    lib.any (a: lib.hasInfix needle a.message) (failed system);
in {
  # Evaluating the module must accept a normal config, reject store paths,
  # and order the tailnet listener with Wants= rather than Requires=.
  eval =
    if !mentions good "backplane" && mentions badFile "/nix/store" && mentions badCred "/nix/store"
    then let
      svc = withTailscale.config.systemd.services.backplane;
    in
      if
        lib.elem "tailscaled.service" svc.wants
        && lib.elem "tailscaled-autoconnect.service" svc.wants
        && lib.elem "tailscaled-autoconnect.service" svc.after
        && !(lib.elem "tailscaled-autoconnect.service" (svc.requires or []))
      then pkgs.runCommand "backplane-eval" {} "touch $out"
      else throw "tailscaled-autoconnect.service was not an optional (Wants=/After=) dependency"
    else throw "Backplane module assertions did not reject /nix/store secrets (or a normal config was rejected)";

  # Equivalent of `backplane --version`, which upstream does not implement.
  version =
    pkgs.runCommand "backplane-version-${version}" {
      nativeBuildInputs = [pkgs.curl];
    } ''
      set -eu
      home=$(mktemp -d)
      rev=${lib.escapeShellArg revision}
      mkdir -p "$home/.cache/backplane/kistack/$rev/skills/pcb"
      mkdir -p "$home/.cache/backplane/kistack/$rev/.claude-plugin"
      printf '%s\n' '{"name":"kistack","description":"fixture"}' > "$home/.cache/backplane/kistack/$rev/.claude-plugin/plugin.json"
      printf '%s\n' '# pcb' > "$home/.cache/backplane/kistack/$rev/skills/pcb/SKILL.md"
      printf '%s\n' "$rev" > "$home/.cache/backplane/kistack/current"
      # The wrapper uses set -u and must tolerate an unset LD_LIBRARY_PATH.
      unset LD_LIBRARY_PATH || true
      sh -n ${package}/bin/backplane
      grep -q 'LD_LIBRARY_PATH:-' ${package}/bin/backplane
      export HOME="$home"
      # --gpu is a Bend runtime flag and has to come before --.
      # --headless without --foreground detaches; the check must wait on this pid.
      ${package}/bin/backplane --gpu off -- --headless --foreground --no-tailscale \
        --home "$home" --host 127.0.0.1 --port 37871 \
        --web ${package}/libexec/backplane/web \
        >"$home/log" 2>&1 &
      pid=$!
      cleanup() {
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
      }
      trap cleanup EXIT
      ok=0
      i=0
      while [ "$i" -lt 90 ]; do
        if curl -sf "http://127.0.0.1:37871/hello" -o "$home/hello"; then
          ok=1
          break
        fi
        if ! kill -0 "$pid" 2>/dev/null; then
          echo "backplane exited before /hello" >&2
          cat "$home/log" >&2
          exit 1
        fi
        i=$((i + 1))
        sleep 1
      done
      if [ "$ok" != 1 ]; then
        echo "timed out waiting for /hello" >&2
        cat "$home/log" >&2
        exit 1
      fi
      if ! grep -a -q ${lib.escapeShellArg version} "$home/hello"; then
        echo "hello did not contain ${version}" >&2
        cat "$home/hello" >&2 || true
        exit 1
      fi
      if ! grep -a -q backplane "$home/hello"; then
        echo "hello did not contain the backplane key" >&2
        cat "$home/hello" >&2 || true
        exit 1
      fi
      if [ ! -s "$home/token" ]; then
        echo "pairing token file was not written" >&2
        exit 1
      fi
      tok=$(tr -d '\n' < "$home/token")
      if grep -a -F "$tok" "$home/log"; then
        echo "pairing token was printed with --no-tailscale" >&2
        exit 1
      fi
      mapped=0
      i=0
      while [ "$i" -lt 20 ]; do
        if grep -q 'libfreetype.so.6' "/proc/$pid/maps" \
          && grep -q 'libfontconfig.so.1' "/proc/$pid/maps" \
          && grep -q 'libX11.so.6' "/proc/$pid/maps"; then
          mapped=1
          break
        fi
        i=$((i + 1))
        sleep 1
      done
      if [ "$mapped" != 1 ]; then
        echo "dlopen/NEEDED libraries were not mapped" >&2
        grep -E 'libfreetype|libfontconfig|libX11' "/proc/$pid/maps" >&2 || true
        cat "$home/log" >&2
        exit 1
      fi
      printf '%s\n' "backplane ${version}" > "$out"
    '';

  # runNixOSTest is the module interface, so requiredFeatures.kvm can be
  # turned off. testers.nixosTest's legacy makeTest does not accept it.
  nixos = pkgs.testers.runNixOSTest {
    name = "backplane";
    # Do not require the kvm system feature. QEMU is still invoked with
    # accel=kvm:tcg, so a builder without KVM emulates.
    requiredFeatures.kvm = false;
    nodes.machine = {
      imports = [module];
      system.stateVersion = "26.05";
      documentation.enable = false;
      documentation.man.enable = false;
      documentation.doc.enable = false;
      documentation.nixos.enable = false;

      # Builders often have no usable KVM. forceAccel defaults false, so
      # qemu is invoked with accel=kvm:tcg and falls back to TCG.
      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      virtualisation.qemu.forceAccel = false;

      environment.systemPackages = [
        pkgs.curl
        pkgs.iproute2
        pkgs.procps
      ];

      services.backplane = {
        enable = true;
        package = package;
        host = "127.0.0.1";
        port = 3787;
        openFirewall = false;
        tailscale.enable = false;
        environmentFile = "/var/lib/backplane-secret/env";
        credentials.BACKPLANE_TEST_TOKEN = "/var/lib/backplane-secret/token";
        kistack.source = fixture;
        kistack.revision = revision;
        kistack.readOnly = true;
      };

      systemd.tmpfiles.rules = [
        "d /var/lib/backplane-secret 0700 root root - -"
        "f /var/lib/backplane-secret/token 0400 root root - super-secret-token"
        "f /var/lib/backplane-secret/env 0400 root root - BACKPLANE_FROM_FILE=from-file"
      ];
    };

    testScript = ''
      start_all()
      # linger-users is a oneshot without RemainAfterExit, so it is inactive
      # after it succeeds. The linger file is the thing that matters.
      machine.wait_for_unit("multi-user.target")
      machine.wait_for_unit("backplane.service")
      machine.wait_for_open_port(3787)
      machine.succeed("curl -sf http://127.0.0.1:3787/hello -o /tmp/hello")
      machine.succeed("grep -a -q ${version} /tmp/hello")
      machine.succeed("grep -a -q backplane /tmp/hello")

      machine.succeed("ss -ltn > /tmp/ss.txt")
      machine.succeed("grep -q '127.0.0.1:3787' /tmp/ss.txt")
      machine.succeed("! grep -E '0\\.0\\.0\\.0:3787|\\[::\\]:3787|\\*:3787' /tmp/ss.txt")

      machine.succeed("test \"$(stat -c %U /var/lib/backplane)\" = backplane")
      machine.succeed("test \"$(stat -c %U /var/cache/backplane)\" = backplane")
      machine.succeed("test -f /var/lib/systemd/linger/backplane")

      # --no-tailscale: upstream does not print the token. With the tailnet
      # listener on, the same line is left in the journal as printed.
      machine.succeed("journalctl -u backplane -o cat --no-pager > /tmp/journal.txt")
      machine.succeed("! grep -F \"$(cat /var/lib/backplane/token)\" /tmp/journal.txt")

      machine.succeed("grep -a -z -x -F 'BACKPLANE_TEST_TOKEN=super-secret-token' /proc/$(pidof -s backplane-serve)/environ")
      machine.succeed("grep -a -z -x -F 'BACKPLANE_FROM_FILE=from-file' /proc/$(pidof -s backplane-serve)/environ")
      machine.succeed("grep -a -z -x -F 'XDG_CACHE_HOME=/var/cache/backplane' /proc/$(pidof -s backplane-serve)/environ")
      machine.succeed("grep -a -z -x -F 'HOME=/var/lib/backplane' /proc/$(pidof -s backplane-serve)/environ")
      machine.succeed("grep -a -z -x -F 'BACKPLANE_NO_UPDATE=1' /proc/$(pidof -s backplane-serve)/environ")

      machine.succeed("systemctl cat backplane-secrets.path | grep -q 'PathChanged=/var/lib/backplane-secret/token'")
      machine.succeed("systemctl cat backplane-secrets.path | grep -q 'PathChanged=/var/lib/backplane-secret/env'")
      machine.succeed("! systemctl cat backplane-secrets.path | grep -q PathExists")
      machine.succeed("! systemctl show backplane.service -p Wants | grep -q tailscaled")

      machine.succeed("test -f /var/lib/backplane/.cache/backplane/kistack/${revision}/skills/pcb/SKILL.md")
      machine.succeed("test \"$(stat -c %U /var/lib/backplane/.cache/backplane/kistack)\" = root")
      machine.succeed("su -s /bin/sh backplane -c 'test -r /var/lib/backplane/.cache/backplane/kistack/${revision}/skills/pcb/SKILL.md'")
      machine.succeed("su -s /bin/sh backplane -c 'test ! -w /var/lib/backplane/.cache/backplane/kistack'")

      machine.succeed("systemctl --failed --no-legend --plain > /tmp/failed.txt")
      machine.succeed("! grep -q . /tmp/failed.txt")
    '';
  };
}
