# Official release binaries, not a from-source Bend build.
# The window binary NEEDs libX11 and also dlopens libfreetype.so.6,
# libfontconfig.so.1 and libX11.so.6 (src/gfx/effects/font.c, src/app/effects/win.c).
# autoPatchelf fixes NEEDED entries. runtimeDependencies plus an explicit
# rpath cover the dlopen names. The bin/ wrappers also export LD_LIBRARY_PATH
# without reading it unset (`set -u` safe) and set BACKPLANE_NO_UPDATE.
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  libx11,
  libxcb,
  libxau,
  libxdmcp,
  freetype,
  fontconfig,
  sources ? import ./sources.nix,
}: let
  system = stdenv.hostPlatform.system;
  plat =
    if builtins.hasAttr system sources.platforms
    then sources.platforms.${system}
    else throw "Backplane publishes no release binary for ${system}";
  runtimeLibs = [
    freetype
    fontconfig
    libx11
    libxcb
    libxau
    libxdmcp
  ];
  libPath = lib.makeLibraryPath runtimeLibs;
  # @out@ and @libPath@ are filled in during install. A toFile result must
  # not mention a derivation, and ${LD_LIBRARY_PATH:-} must survive `set -u`.
  wrapperFor = name:
    builtins.toFile "backplane-${name}-wrapper" ''
      #!/bin/sh
      set -eu
      export BACKPLANE_NO_UPDATE=1
      if [ -n "''${LD_LIBRARY_PATH:-}" ]; then
        export LD_LIBRARY_PATH=@libPath@:$LD_LIBRARY_PATH
      else
        export LD_LIBRARY_PATH=@libPath@
      fi
      exec @out@/libexec/backplane/${name} "$@"
    '';
in
  stdenv.mkDerivation {
    pname = "backplane";
    version = sources.version;

    src = fetchurl {
      inherit (plat) url hash;
      name = "backplane-${sources.version}-${plat.dir}.tar.gz";
    };
    sourceRoot = "backplane-${plat.dir}";

    nativeBuildInputs = [autoPatchelfHook];
    buildInputs = runtimeLibs;
    # dlopen() searches the binary's RUNPATH. These are not all NEEDED.
    runtimeDependencies = runtimeLibs;
    # The bun helpers list the dynamic linker as NEEDED. It is the interpreter,
    # not a library autoPatchelf can add.
    autoPatchelfIgnoreMissingDeps = [
      "ld-linux-x86-64.so.2"
      "ld-linux-aarch64.so.1"
    ];

    inherit libPath;

    dontStrip = true;
    dontConfigure = true;
    dontBuild = true;

    installPhase = ''
      runHook preInstall
      libexec=$out/libexec/backplane
      mkdir -p "$libexec" "$out/bin" \
        "$out/share/applications" \
        "$out/share/icons/hicolor/scalable/apps" \
        "$out/share/icons/hicolor/256x256/apps" \
        "$out/share/licenses/backplane"

      for b in backplane backplane-serve backplane-browser backplane-step2glb backplane-voice; do
        if [ -e "$b" ]; then
          cp -a "$b" "$libexec/$b"
          chmod 755 "$libexec/$b"
        fi
      done
      cp -a web "$libexec/web"
      if [ -d licenses ]; then
        cp -a licenses "$out/share/licenses/backplane/licenses"
      fi
      if [ -f LICENSE ]; then
        cp -a LICENSE "$out/share/licenses/backplane/LICENSE"
      fi
      if [ -f backplane.desktop ]; then
        sed "s|^Exec=.*|Exec=$out/bin/backplane|" backplane.desktop > "$out/share/applications/backplane.desktop"
      fi
      if [ -f icon/backplane.svg ]; then
        cp -a icon/backplane.svg "$out/share/icons/hicolor/scalable/apps/backplane.svg"
      fi
      if [ -f icon/backplane-256.png ]; then
        cp -a icon/backplane-256.png "$out/share/icons/hicolor/256x256/apps/backplane.png"
      fi

      test -x "$libexec/backplane"
      test -x "$libexec/backplane-serve"
      substitute ${wrapperFor "backplane"} "$out/bin/backplane" --subst-var out --subst-var libPath
      substitute ${wrapperFor "backplane-serve"} "$out/bin/backplane-serve" --subst-var out --subst-var libPath
      chmod 755 "$out/bin/backplane" "$out/bin/backplane-serve"
      runHook postInstall
    '';

    # autoPatchelf records NEEDED libraries. Append the dlopen directories
    # as well so a RUNPATH lookup finds them even if the hook did not.
    postFixup = ''
      for b in backplane backplane-serve; do
        bin="$out/libexec/backplane/$b"
        [ -e "$bin" ] || continue
        old=$(patchelf --print-rpath "$bin")
        if [ -n "$old" ]; then
          patchelf --set-rpath "$old:${libPath}" "$bin"
        else
          patchelf --set-rpath "${libPath}" "$bin"
        fi
      done
    '';

    passthru = {
      updateScript = ./update.sh;
      inherit sources;
    };

    meta = {
      description = "Agentic hardware development, with live KiCad viewers";
      longDescription = ''
        Prebuilt Backplane hub and window from the upstream GitHub release.
        backplane-step2glb embeds OpenCascade (LGPL-2.1); its notices are in
        share/licenses. The package sets BACKPLANE_NO_UPDATE so a packaged
        install does not replace itself from GitHub.
      '';
      homepage = "https://github.com/i2cjak/Backplane";
      license = with lib.licenses; [
        mit
        lgpl21
      ];
      sourceProvenance = with lib.sourceTypes; [binaryNativeCode];
      platforms = builtins.attrNames sources.platforms;
      mainProgram = "backplane";
    };
  }
