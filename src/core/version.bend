import Base

# The running build's version. Version.slot is a slot: a build writes its
# version over it in the C bend emitted (scripts/stamp-version.py, run by
# scripts/build-app.sh with BACKPLANE_VERSION_STAMP): 40 characters, padded
# with "~", then a terminator no other string ends in (bend shares equal
# tails between literals; this keeps every cell of the slot its own). The
# version is no input of the emit, so a tree emits the same C whatever its
# version. Unstamped (tests, the JS builds) it reads 0.1.0-dev.

def Version.slot() -> String:
  "0.1.0-dev~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~@BPVERSIONSLOT"

def Version.current() -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(Version.slot(), '~')), "")
