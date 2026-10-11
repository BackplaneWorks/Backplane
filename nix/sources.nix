# Pins for the official release binaries. nix/update.sh rewrites this from
# GitHub's SHA256SUMS and from the tag's KiStack.revision().
{
  version = "0.11.1";
  # src/server/kicad.bend KiStack.revision() at this tag.
  kistackRevision = "ecf16c119c7ed610f5f251c479b03884b8d98b09";
  platforms = {
    aarch64-linux = {
      dir = "linux-arm64";
      url = "https://github.com/i2cjak/Backplane/releases/download/v0.11.1/backplane-v0.11.1-linux-arm64.tar.gz";
      hash = "sha256-AeplHZ3phngRHcRB12M3o5aMIj8r4KOsMl9D6vy9HF0=";
    };
    x86_64-linux = {
      dir = "linux-x64";
      url = "https://github.com/i2cjak/Backplane/releases/download/v0.11.1/backplane-v0.11.1-linux-x64.tar.gz";
      hash = "sha256-J7XBGkqq5N0aGTBs2nFDD9BsELtvLXbhuk25Lh4b6yk=";
    };
  };
}
