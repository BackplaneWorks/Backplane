import Base

# The running build's version. Version.slot is a slot: a build writes its
# version over it in the JS bend emitted (scripts/stamp-version.py, run by
# scripts/build-js.sh with BACKPLANE_VERSION_STAMP): 40 characters, padded
# with "~", then a terminator no other string ends in (bend shares equal
# tails between literals; this keeps every cell of the slot its own). The
# version is no input of the emit, so a tree emits the same JS whatever its
# version. Unstamped (tests) it reads 0.1.0-dev.

def Version.slot() -> String:
  "0.1.0-dev~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~@BPVERSIONSLOT"

def Version.current() -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Version.slot(), '~')), "")

# The commit the build was made from, in a slot of its own the same way
# (scripts/stamp-version.py FILE SHA commit, run by scripts/build-js.sh with
# BACKPLANE_COMMIT_STAMP): a main-track build reports its version as the
# release the installed desktop app requires, and its commit beside it (GET
# /hello's "commit"). Unstamped it reads "unknown".
def Version.commit.slot() -> String:
  "unknown~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~@BPCOMMITSLOT"

def Version.commit() -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Version.commit.slot(), '~')), "")
