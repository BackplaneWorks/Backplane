{
  description = "Backplane, packaged from the official Linux release";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = f:
      nixpkgs.lib.genAttrs systems (
        system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [self.overlays.default];
            }
          )
      );
  in {
    overlays.default = final: _prev: {
      backplane = final.callPackage ./nix/package.nix {};
    };

    packages = forAllSystems (pkgs: {
      default = pkgs.backplane;
      inherit (pkgs) backplane;
    });

    apps = forAllSystems (pkgs: {
      default = {
        type = "app";
        program = "${pkgs.backplane}/bin/backplane";
        meta.description = "Run the Backplane window and hub";
      };
    });

    # `nix fmt` with no arguments feeds stdin to a bare alejandra, which
    # then errors. Format the flake root (or the paths nix forwards).
    formatter = forAllSystems (
      pkgs:
        pkgs.writeShellScriptBin "alejandra" ''
          if [ "$#" -eq 0 ]; then
            set -- "''${PRJ_ROOT:-.}"
          fi
          exec ${pkgs.alejandra}/bin/alejandra "$@"
        ''
    );

    nixosModules.default = import ./nix/module.nix;

    checks = forAllSystems (
      pkgs:
        import ./nix/checks.nix {
          inherit pkgs;
          inherit (nixpkgs) lib;
          inherit nixpkgs;
          module = self.nixosModules.default;
          package = pkgs.backplane;
          sources = import ./nix/sources.nix;
        }
    );
  };
}
