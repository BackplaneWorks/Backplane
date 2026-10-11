# NixOS module for the Backplane hub.
#
# What this follows, checked against the sources (not guessed):
# - Flags: --home, --host (default 127.0.0.1), --port (default 3787),
#   --web, --no-tailscale, --tailscale, --tailscale-https. An unknown flag
#   is ignored. A non-numeric --port falls back to 3787. There is no
#   --version; asking a running hub's GET /hello is how you read the version
#   (CBOR {"backplane": "<version>", "busy": N}).
# - BACKPLANE_NO_UPDATE (any non-empty value) or APPIMAGE turns the
#   self-updater off. There is no home/tailscale-path/kistack env var.
# - The pairing token is 128 hex chars in <home>/token (mode 0600). It is
#   printed in full on stdout only when the tailnet listener binds:
#   "pairing token <hex>" and, for --tailscale-https, "<url>/#token=<hex>".
#   The unit does not filter that line; the journal gets what the binary prints.
# - KiStack is hardcoded to $HOME/.cache/backplane/kistack, not XDG_CACHE_HOME.
#   Startup writes .claude-plugin/plugin.json into the revision directory and
#   then refreshes GitHub HEAD into a `current` file. The next start prefers
#   `current`. There is no switch to keep a pin or to use a read-only tree.
# - `tailscale` is executed by name. The tailnet address is read once at
#   start (`tailscale ip -4`); an empty answer stays "unavailable" until
#   restart. Peer discovery retries every two minutes.
# - Agents are spawned by bare name (claude, codex, grok) with the hub's
#   environment. `systemd-run --user` is used when `systemctl --user
#   show-environment` works, otherwise the agent is unconfined.
# - backplane --headless detaches unless --foreground is also passed.
#   backplane-serve stays in the foreground and treats a taken port as fatal.
#   The service runs backplane-serve.
{
  config,
  lib,
  pkgs,
  ...
}: let
  sources = import ./sources.nix;
  cfg = config.services.backplane;
  secretPaths =
    lib.optional (cfg.environmentFile != null) cfg.environmentFile ++ lib.attrValues cfg.credentials;
  storeSecrets = lib.filter (p: lib.hasPrefix "/nix/store" p) secretPaths;
  relativeSecrets = lib.filter (p: !lib.hasPrefix "/" p) secretPaths;
  tailscalePackage =
    if cfg.tailscale.package != null
    then cfg.tailscale.package
    else if config.services.tailscale.enable
    then config.services.tailscale.package
    else pkgs.tailscale;
  basePackages = with pkgs; [
    bash
    coreutils
    curl
    findutils
    gawk
    gnugrep
    gnused
    gnutar
    gzip
    procps
    systemd
    util-linux
  ];
  serveArgs =
    [
      "${cfg.package}/bin/backplane-serve"
      "--home"
      cfg.stateDir
      "--host"
      cfg.host
      "--port"
      (toString cfg.port)
      "--web"
      "${cfg.package}/libexec/backplane/web"
    ]
    ++ lib.optionals (!cfg.tailscale.enable) ["--no-tailscale"]
    ++ lib.optionals (cfg.tailscale.enable && cfg.tailscale.https) ["--tailscale-https"]
    ++ cfg.extraArgs;
  runScript = pkgs.writeShellScript "backplane-run" ''
    set -euo pipefail
    cred_dir="''${CREDENTIALS_DIRECTORY:-}"
    if [ -n "$cred_dir" ]; then
      for f in "$cred_dir"/*; do
        [ -f "$f" ] || continue
        name=$(basename "$f")
        case "$name" in
          *[!A-Za-z0-9_]*) continue ;;
          [0-9]*) continue ;;
        esac
        # A credential file is one variable's value. Trailing newlines are
        # stripped, matching how a token file is usually written.
        val=$(cat "$f")
        export "$name=$val"
      done
    fi
    exec ${lib.escapeShellArgs serveArgs}
  '';
  seedScript = pkgs.writeShellScript "backplane-kistack-seed" ''
    set -eu
    home=${lib.escapeShellArg cfg.home}
    rev=${lib.escapeShellArg cfg.kistack.revision}
    source=${lib.escapeShellArg cfg.kistack.source}
    user=${lib.escapeShellArg cfg.user}
    group=${lib.escapeShellArg cfg.group}
    readonly=${
      if cfg.kistack.readOnly
      then "1"
      else "0"
    }
    base="$home/.cache/backplane/kistack"
    dest="$base/$rev"
    stamp="$base/.source"
    mkdir -p "$base"
    if [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$source" ] && [ -f "$dest/skills/pcb/SKILL.md" ]; then
      :
    else
      chmod -R u+w "$base" 2>/dev/null || true
      rm -rf "$dest"
      mkdir -p "$dest"
      cp -a "$source/." "$dest/"
      printf '%s\n' "$source" > "$stamp"
    fi
    if [ ! -f "$dest/.claude-plugin/plugin.json" ]; then
      chmod -R u+w "$dest" 2>/dev/null || true
      mkdir -p "$dest/.claude-plugin"
      printf '%s\n' '{"name":"kistack","description":"KiStack KiCad skills by American Embedded"}' > "$dest/.claude-plugin/plugin.json"
    fi
    if [ "$readonly" = 1 ]; then
      # A later boot finds the tree mode a-w. Rewrite current only when it
      # does not already name this revision, and only after making it writable.
      cur=""
      if [ -f "$base/current" ]; then
        cur=$(cat "$base/current" 2>/dev/null || true)
      fi
      if [ "$cur" != "$rev" ]; then
        chmod -R u+w "$base" 2>/dev/null || true
        printf '%s\n' "$rev" > "$base/current"
      fi
      chown -R root:root "$base"
      chmod -R a+rX "$base"
      chmod -R a-w "$base"
    else
      if [ ! -f "$base/current" ]; then
        printf '%s\n' "$rev" > "$base/current"
      fi
      chown -R "$user:$group" "$base"
    fi
  '';
  dirRule = path: mode: "d ${path} ${mode} ${cfg.user} ${cfg.group} - -";
in {
  options.services.backplane = {
    enable = lib.mkEnableOption "the Backplane hub";

    package = lib.mkOption {
      type = lib.types.package;
      default =
        if pkgs ? backplane
        then pkgs.backplane
        else pkgs.callPackage ./package.nix {};
      defaultText = lib.literalExpression "pkgs.backplane";
      description = ''
        Package that provides bin/backplane, bin/backplane-serve and
        libexec/backplane/web. The service runs backplane-serve.
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "backplane";
      description = "Account the hub runs as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = cfg.user;
      defaultText = lib.literalExpression "user";
      description = "Primary group of the hub account.";
    };

    createUser = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Create `user` and `group` as a system account. The account lingers
        so a user manager is running and upstream can put each agent in a
        `systemd-run --user` scope. Set false when the account already exists;
        give that account linger yourself or agents run unconfined.
      '';
    };

    linger = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        When createUser is set, enable lingering for the account
        (`loginctl enable-linger`). Upstream's memory cap and durable
        `work_run` units need `systemctl --user`.
      '';
    };

    stateDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/backplane";
      description = ''
        Passed as `--home`. The event log, pairing token and secrets live here.
      '';
    };

    home = lib.mkOption {
      type = lib.types.str;
      default = cfg.stateDir;
      defaultText = lib.literalExpression "stateDir";
      description = ''
        HOME for the process. KiStack is hardcoded to
        `$HOME/.cache/backplane/kistack` (it does not read XDG_CACHE_HOME),
        so the default keeps that tree on the state directory.
      '';
    };

    cacheDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/cache/backplane";
      description = ''
        XDG_CACHE_HOME. Upstream honors it for model and KiCad-discovery
        caches (`$XDG_CACHE_HOME/backplane-bend/...`). KiStack does not.
      '';
    };

    host = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = ''
        Address passed as `--host`. The hub binds this one address.
        `0.0.0.0` listens on every IPv4 interface. The tailnet listener,
        when enabled, is a second socket on the tailscale address.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 3787;
      description = ''
        TCP port. The module passes a number; upstream would silently use
        3787 if `--port` were not numeric.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Open `port` in the firewall. With the default loopback host this
        does not make the hub reachable from other machines.
      '';
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [];
      example = lib.literalExpression "with pkgs; [ git claude-code kicad freecad codex ]";
      description = ''
        Extra programs on the hub's PATH. Agents are started by bare name
        (`claude`, `codex`, `grok`), KiCad via `kicad-cli` or
        BACKPLANE_KICAD_CLI, FreeCAD via `freecadcmd` or BACKPLANE_FREECAD,
        and git is used for thread worktrees. The module always adds the
        tools the hub's own shell scripts call (curl, bash, coreutils,
        grep, sed, awk, find, tar, gzip, ps, systemd-run).
      '';
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = ''
        Extra arguments after the managed ones. Unknown flags are ignored
        by the hub.
      '';
    };

    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {};
      example = {
        BACKPLANE_KICAD_CLI = "/run/current-system/sw/bin/kicad-cli";
        BACKPLANE_FREECAD = "/run/current-system/sw/bin/freecadcmd";
      };
      description = ''
        Extra environment variables for the hub and, because agents inherit
        the hub's environment, for agents. Not for secrets: a value written
        here is copied into the system closure. Use environmentFile or
        credentials for tokens.
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/run/secrets/backplane.env";
      description = ''
        Absolute path of a systemd EnvironmentFile (KEY=value lines).
        Must not point into the Nix store. A path literal in a Nix file
        would be copied into the store and then rejected. systemd refuses
        to start the service if the file is missing.
      '';
    };

    credentials = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {};
      example = {
        CLAUDE_CODE_OAUTH_TOKEN = "/run/secrets/claude-token";
        OPENAI_API_KEY = "/run/secrets/openai-key";
      };
      description = ''
        Credential files loaded with LoadCredential and exported to the hub
        under the attribute name. Each path is absolute and outside the Nix
        store. The file's contents are the variable's value, with trailing
        newlines removed. Agents inherit the hub environment, and upstream
        does not re-read it later, so a change restarts the service (a
        path unit on PathChanged, not PathExists).
      '';
    };

    tailscale = {
      enable = lib.mkEnableOption ''
        the tailnet listener. Adds tailscale to PATH and orders the service
        after tailscaled.service and, if it exists, tailscaled-autoconnect.service
        (Wants=, so a missing autoconnect unit does not fail the boot).
        Upstream decides the tailnet address once at start
      '';

      https = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Pass `--tailscale-https` (`tailscale serve` on 443). Implies the
          tailnet listener. The pairing URL this prints contains the token,
          and that line is written to the journal as the binary prints it.
        '';
      };

      package = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        default = null;
        description = ''
          tailscale package placed on PATH when the tailnet listener is on.
          null uses services.tailscale.package when that service is enabled,
          otherwise pkgs.tailscale.
        '';
      };
    };

    kistack = {
      source = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          KiStack tree to seed (the repository root, containing
          skills/pcb/SKILL.md). Upstream has no read-only mode and no
          directory override: it uses `$HOME/.cache/backplane/kistack`,
          writes `.claude-plugin/plugin.json` into the revision, and a
          background refresh records GitHub HEAD in `current`, which the
          next start prefers. The module copies this tree into that cache
          (a copy, because the hub writes into the tree). Leave null and
          the hub downloads the pin itself, which needs curl and a network.
        '';
      };

      revision = lib.mkOption {
        type = lib.types.str;
        default = sources.kistackRevision;
        defaultText = lib.literalExpression "the KiStack.revision() pin for this Backplane version";
        description = ''
          Directory name under the KiStack cache. Defaults to the revision
          pinned by the Backplane sources this package was built against.
        '';
      };

      readOnly = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          After seeding, make the KiStack cache non-writable by the service
          user and point `current` at `revision`. Upstream's refresh cannot
          replace the pin; the failed writes are ignored. Set false to let
          a later start follow GitHub HEAD. Has no effect when source is null.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = builtins.length storeSecrets == 0;
        message = "services.backplane refuses secret paths in /nix/store: ${lib.concatStringsSep ", " storeSecrets}";
      }
      {
        assertion = builtins.length relativeSecrets == 0;
        message = "services.backplane secret paths must be absolute: ${lib.concatStringsSep ", " relativeSecrets}";
      }
    ];

    warnings = lib.optional (cfg.openFirewall && (cfg.host == "127.0.0.1" || cfg.host == "::1" || cfg.host == "localhost")) ''
      services.backplane.openFirewall is set, but host ${cfg.host} is loopback, so other machines still cannot connect.
    '';

    users.groups.${cfg.group} = lib.mkIf cfg.createUser {};
    users.users.${cfg.user} = lib.mkIf cfg.createUser {
      isSystemUser = true;
      group = cfg.group;
      home = cfg.home;
      createHome = false;
      linger = cfg.linger;
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [cfg.port];

    systemd.tmpfiles.rules = lib.unique [
      (dirRule cfg.stateDir "0750")
      (dirRule cfg.cacheDir "0750")
      (dirRule cfg.home "0750")
      (dirRule "${cfg.home}/.cache" "0750")
      (dirRule "${cfg.home}/.cache/backplane" "0750")
      (dirRule "${cfg.home}/.cache/backplane/kistack" "0750")
      (dirRule "${cfg.home}/.config" "0750")
      (dirRule "${cfg.stateDir}/share" "0750")
      (dirRule "${cfg.stateDir}/config" "0750")
      (dirRule "${cfg.stateDir}/secrets" "0700")
    ];

    systemd.services.backplane-kistack = lib.mkIf (cfg.kistack.source != null) {
      description = "Seed Backplane's KiStack cache";
      wantedBy = ["multi-user.target"];
      before = ["backplane.service"];
      requiredBy = ["backplane.service"];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = seedScript;
      };
    };

    systemd.paths.backplane-secrets = lib.mkIf (secretPaths != []) {
      description = "Restart Backplane when a credential file changes";
      wantedBy = ["multi-user.target"];
      pathConfig.PathChanged = secretPaths;
    };

    systemd.services.backplane-secrets = lib.mkIf (secretPaths != []) {
      description = "Restart Backplane when a credential file changes";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${pkgs.systemd}/bin/systemctl restart backplane.service";
      };
    };

    systemd.services.backplane = {
      description = "Backplane hub";
      wantedBy = ["multi-user.target"];
      wants =
        ["network-online.target"]
        ++ lib.optionals cfg.tailscale.enable [
          "tailscaled.service"
          "tailscaled-autoconnect.service"
        ];
      after =
        ["network-online.target"]
        ++ lib.optionals cfg.tailscale.enable [
          "tailscaled.service"
          "tailscaled-autoconnect.service"
        ]
        ++ lib.optional (cfg.kistack.source != null) "backplane-kistack.service"
        ++ lib.optional (cfg.createUser && cfg.linger) "linger-users.service";
      requires = lib.optional (cfg.kistack.source != null) "backplane-kistack.service";

      path =
        cfg.extraPackages
        ++ lib.optional cfg.tailscale.enable tailscalePackage
        ++ basePackages
        ++ [cfg.package];

      environment =
        {
          HOME = cfg.home;
          XDG_CACHE_HOME = cfg.cacheDir;
          XDG_STATE_HOME = cfg.stateDir;
          XDG_DATA_HOME = "${cfg.stateDir}/share";
          XDG_CONFIG_HOME = "${cfg.home}/.config";
          # %U is the uid of User=. The user manager's bus is what
          # `systemctl --user` looks at for agent scopes.
          XDG_RUNTIME_DIR = "/run/user/%U";
          DBUS_SESSION_BUS_ADDRESS = "unix:path=/run/user/%U/bus";
          BACKPLANE_NO_UPDATE = "1";
        }
        // cfg.environment;

      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.home;
        ExecStart = runScript;
        Restart = "on-failure";
        RestartSec = 3;
        TimeoutStartSec = 180;
        TimeoutStopSec = 30;
        KillMode = "control-group";
        # The hub prints the pairing token when the tailnet listener binds.
        # Leave that line in the journal; do not filter it.
        StandardOutput = "journal";
        StandardError = "journal";
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;
        LoadCredential = lib.mapAttrsToList (name: path: "${name}:${path}") cfg.credentials;
        StateDirectory = lib.mkIf (cfg.stateDir == "/var/lib/backplane") "backplane";
        StateDirectoryMode = lib.mkIf (cfg.stateDir == "/var/lib/backplane") "0750";
        CacheDirectory = lib.mkIf (cfg.cacheDir == "/var/cache/backplane") "backplane";
        CacheDirectoryMode = lib.mkIf (cfg.cacheDir == "/var/cache/backplane") "0750";
        UMask = "0077";
        LimitNOFILE = 1048576;
        TasksMax = "infinity";
        # Agents are arbitrary CLIs (JIT, network, project dirs, user scopes).
        # ProtectHome would make project folders read-only. MemoryDenyWriteExecute
        # would break their JITs. ProtectControlGroups and a tight syscall
        # filter would break `systemd-run --user`. PrivateDevices stays off
        # so KiCad/FreeCAD can see a GPU node when one exists.
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = false;
        PrivateTmp = true;
        PrivateDevices = false;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        SystemCallArchitectures = "native";
        ReadWritePaths = lib.unique [
          cfg.stateDir
          cfg.home
          cfg.cacheDir
          "${cfg.home}/.cache"
          "${cfg.home}/.config"
        ];
      };
    };
  };
}
