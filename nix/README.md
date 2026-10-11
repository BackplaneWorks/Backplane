# Nix / NixOS

The flake packages the official Linux release binaries (`x86_64-linux` and `aarch64-linux`) from [i2cjak/Backplane](https://github.com/i2cjak/Backplane). It does not compile Backplane. nixpkgs is pinned to `nixos-26.05`. `packages.default` is `backplane`; `overlays.default` adds that package; `apps.default` runs it; `nix fmt` is alejandra.

```sh
nix run .          # the window binary; a hub starts with it
nix build
nix flake check
```

There is no `--version`. An unknown flag is ignored, and a hub would start on the default home. The version is the `backplane` field of `GET /hello`, which is CBOR, not a text banner. The flake's `version` check starts the packaged binary and reads that.

A NixOS configuration:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  inputs.backplane.url = "github:Riceman2000/Backplane";

  outputs = { nixpkgs, backplane, ... }: {
    nixosConfigurations.box = nixpkgs.lib.nixosSystem {
      modules = [
        backplane.nixosModules.default
        { nixpkgs.overlays = [ backplane.overlays.default ]; }
        ({ pkgs, ... }: {
          services.backplane = {
            enable = true;
            # Loopback unless you mean to publish the hub.
            host = "127.0.0.1";
            port = 3787;
            openFirewall = false;
            tailscale.enable = true;
            extraPackages = [ pkgs.git pkgs.kicad pkgs.freecad ];
            # Strings, not path literals. A path would be copied into the
            # Nix store, and the module rejects store paths.
            environmentFile = "/run/secrets/backplane.env";
            credentials.CLAUDE_CODE_OAUTH_TOKEN = "/run/secrets/claude-token";
            # A tree containing skills/pcb/SKILL.md. Omit it and the hub
            # downloads the pin itself (needs a network).
            # kistack.source = ./kistack;
            # kistack.readOnly = true;
          };
        })
      ];
    };
  };
}
```

The service runs `backplane-serve` (it stays in the foreground; `backplane --headless` without `--foreground` detaches). State is `--home` (`/var/lib/backplane` by default), owned by the `backplane` user. `XDG_CACHE_HOME` is `/var/cache/backplane`. The package and the unit set `BACKPLANE_NO_UPDATE=1`. The account lingers so upstream can put each agent in a `systemd-run --user` scope; without a user manager those agents run unconfined.

`tailscale.enable` puts `tailscale` on `PATH` and orders the service after `tailscaled.service` and `tailscaled-autoconnect.service` with `Wants=`, so a machine without the autoconnect unit still boots. Upstream reads `tailscale ip -4` once at start.

KiStack is hardcoded to `$HOME/.cache/backplane/kistack` (not `XDG_CACHE_HOME`). Upstream writes into that tree and, on a later start, prefers whatever `current` names, which a background refresh sets to GitHub HEAD. There is no upstream switch for a pin or a read-only tree. `kistack.source` is copied into that cache; `kistack.readOnly` (the default) then points `current` at `kistack.revision` and removes write permission. The refresh's failed writes are ignored.

The pairing token is 128 hex characters in `<home>/token` (mode 0600). Upstream prints it on stdout only when the tailnet listener binds (`pairing token <hex>`, and `#token=<hex>` with `--tailscale-https`). The service leaves that line in the journal. With `--no-tailscale` the token is not printed.

`environmentFile` and `credentials` must be absolute paths outside `/nix/store`. Credential files are loaded with `LoadCredential` and exported under the attribute name. A `PathChanged` path unit (not `PathExists`) restarts the hub when one of those files changes. Agents inherit the hub environment from when they were spawned; upstream does not re-read it.

To bump the pin, from a checkout:

```sh
nix/update.sh            # latest stable GitHub release
nix/update.sh v0.11.1    # one tag
nix fmt
```

`nix/update.sh` rewrites `nix/sources.nix` from that release's `SHA256SUMS` and from the tag's `KiStack.revision()` in `src/server/kicad.bend`. It needs `curl` and `python3`.
