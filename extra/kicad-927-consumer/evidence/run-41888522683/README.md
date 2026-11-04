# Native attempt blocked before application launch

GitHub run: https://github.com/BackplaneWorks/Backplane/actions/runs/41888522683

Consumer tools commit: e472e26e95e4f380c6c9abc10a92f339d32b6b4f.
Producer run: 41886983230, authenticated artifact 13352107148.
Application ZIP SHA256: de613310949b72b0a70bebf9a07aff021cdea98f894856fabdc42134d678cd22.

The real Windows Job Object enforced 8,589,934,592 bytes with flag 512
(job memory limit), no breakaway flags, and confirmed process membership.
The Windows 2025 hosted runner reported an active D:\pagefile.sys with
2,944 MiB allocated, current usage 0 and peak usage 0. The hard swap=0
prerequisite therefore refused execution. Final measured job peak was
91,090,944 bytes. The original bootstrap.json is preserved unchanged.

No package extraction, KiCad fixture setup, standard-user creation,
application launch or KiCad check ran. This is neither a reproduced
application defect nor a native PASS. The later requested canonical #946
native measurement was not part of this frozen workflow and also did not run.
Its separately pinned measurement is prepared for a justified pre-launch
recovery after the resource constraint is resolved. No proof, package BUILD
status, source, compiler provenance or release qualification was changed.

Issue #927 remains open. Publication remains on user-confirmation hold.
