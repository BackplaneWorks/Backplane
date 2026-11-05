# KiCad installation completion on Windows (#927)

The current 0.14.2 Windows package, source be904cd4cb21ad6febe17e674918c96945113d07,
tree 1034e9dc35fb517f1186ebcdb2d8f3351beb7a59, failed combined native
standard-user acceptance in GitHub run 41958094050. ZIP SHA256:
32a7685dc41c8647f19340f012e79e8e94fa2e08e338bdf1a756809feb78a563;
service SHA256: d0cb5b3b2d572a072cd41d639147ca93e9b1c1e981272ee7fce82735c5572bc4.

The actual in-app download/extraction completed in 22.809 seconds. The genuine
installed bp.6 CLI had SHA256 23b81e721e0a38ec8c277837badf4507e5518c4f5ddc9f89ec3d83a7a9ae2d43
and its native `version` command printed 10.99.0. The canonical #946 native
msvcrt measurement passed on the current source and service. The health cache
was valid JSON, but remained the pre-install missing-runtime snapshot.

The preserved production diagnostic records hub PID 4092 exiting with code 1
and an Error at epoch 1793847451, when the installation finished. The event
log records clearing `update.restart` at the same instant. Source tracing
identifies the sequence: Install.done(success) -> Upgrade.done -> UpdateReady
-> reexec -> H.Sys.exec. The JavaScript sys_exec implementation unconditionally
loads POSIX libc through host_libc and calls execve; Windows has neither that
POSIX library nor process-replacement mechanism. This is an installation
completion failure, not evidence that the Unicode installation path is absent.

Installation now sends the existing forced KProbe instead of treating a KiCad
runtime install as a hub upgrade. Runtime discovery, library preparation and
the info broadcast already support selection while the hub runs. Failed
installations retain their error notice and do not probe or request restart.
The application-upgrade restart policy is unchanged.

The focused regression extracts the exact production completion, notification
and channel definitions into a small executable Bend harness. It checks the
success notice and forced probe, failure notices with Windows paths and
Unicode, and absence of follow-up on failure. A control reinstates the old
Upgrade.done call and must fail the forced-probe assertion. It is added to the
existing nightly/release CI KiCad lane; scoped merge validation runs it on Boat.
This harness is not full-hub proof or native corrected-package qualification.

Run 41958094050 used the authorized 8 GiB Job Object, verified membership and
no-breakaway flags, with existing pagefile usage and paging recorded. Peak job
memory was 1199804416 bytes; pagefile usage ended at 204 MiB, peak 206 MiB.
The disposable account was removed. Program Files/PATH/manual-path and OMP
phases were not reached in that run. Its failed verdict remains unchanged.

#927 remains open pending corrected-package native acceptance and public
shipment. No source overlay or package rewrite qualifies a fix. Root owns any
new candidate emission/packaging; the acceptance owner does not rebuild it.

Scoped main-fix validation: Boat j20261105030718-2917373 executed the production
completion harness and old-restart negative control successfully, then the
LAWS/PROOF structural check. Kernel scope enforced at most 3700000000 bytes
with swap 0; reported peak child RSS 265504 KiB, command wall 3.96 seconds.
The transported server.bend SHA256 was
27aa449c131104e75147d3e1ddf5c77143b99a729b11055fa27d3cf87a32f87d;
test SHA256 was 2f27f0dd4e9c1939476ad1745341851fc305e5b89d7bb082f87ae893e9e4c39c.
This is a focused IO result, not a complete small-gate or canonical proof pass.
The previous canonical proof attempt was OOM-killed with no result; no new
proof result or corrected-package result is claimed here.

The person subsequently paused 0.14.2 to prioritize GPUI. This small ordinary
main fix and capped focused unit testing remain permitted; release work,
new Windows campaigns, packaging, rebuilds and publication remain paused.
Original failed-run evidence artifact: 13383134838,
sha256:bc799287ea320ab19d0816567ab2b595145c58b11a2df94eb7bccdbbacd6effb.
