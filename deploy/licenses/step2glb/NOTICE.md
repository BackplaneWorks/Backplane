# backplane-step2glb: Open CASCADE Technology

`backplane-step2glb` (Backplane's STEP mesher, source in `native/step2glb`) is linked statically with Open CASCADE Technology
(OCCT) 7.9.2, © Open CASCADE SAS, under the GNU Lesser General Public License version 2.1 (`LICENSE_LGPL_21.txt`) with Open CASCADE's
exception (`OCCT_LGPL_EXCEPTION.txt`).

OCCT's source, exactly as built: https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_9_2.tar.gz
(sha256 3cd080d3fc33ba0c6c157e110afe3e015859524c4694dbb09812ec9d61595639), configured by `scripts/build-occt.sh`. To relink the program
with another build of OCCT, build `native/step2glb` with `scripts/build-step2glb.sh` (`OCCT_DIR=` names that build).
