# Nitro Sandboxes as a sandbox backend

AWS Nitro Sandboxes (announced 2026-11-09) are Firecracker microVMs that the Nitro Hypervisor isolates, on an EC2 instance you own. Backplane can use them as a third backend, alongside `local` and Boat. The setting `sandbox.nitro` turns it on and is off by default. Nothing in Backplane creates an AWS resource: the parent instance is planned with its cost, and the person makes it.

Code: `native/host/src/sandbox/` (trait, Boat and Nitro backends, `sandbox.status`); `scripts/nitro/` (the parent's side and the guest); `scripts/nitro-ssh.sh`.
Tests: `scripts/cargo-q test --offline sandbox::` (a fake AWS and parent) and `test/nitro/vsock-loop.sh` (the parent-guest protocol through a stand-in vsock).

## What AWS provides (EC2, EKS and ECS guides, read 2026-11-10)

- **Parent.** A Nitro EC2 instance launched with `--enclave-options Enabled=true`. Most families from the 5th generation on work: C/M/R 5–9, Intel, AMD and Graviton. The `.large`/`.medium` sizes, most `.metal`, flex instances and anything with nested virtualization do not.
- **Parent OS.** AL2023 with `kernel6.18` ≥ 6.18.51-120.162. It needs a memory pool reserved at boot (`nitro_enclaves.mempool=<N>G@0`, then a reboot). Ubuntu works through a PPA.
- **VMM.** Firecracker ≥ 1.18 is the only one. The config is the usual Firecracker JSON plus an `enclave` object with `cpu_overcommit`, `direct_mode`, `hotplug_mode` and `debug_mode`. The guest kernel must be an EFI-stub kernel; the AL2023 kernel is one, and it has DMB support for confidential mode. Run it as `firecracker --config-file`, or through its API socket.
- **There is no AWS API for sandboxes.** AWS only sees the instance (`run/describe/start/stop-instances`). Sandboxes are processes on the parent.
- **Limits:**
  - Up to 5 sandboxes per parent vCPU, and at most 896 per parent.
  - At least 256 MiB each, in steps of 2 MiB.
  - Pool cost per sandbox: its memory + 2 MiB, plus 16 MiB per device in confidential mode.
  - 4 slots to start; raise them in steps of 8 through `/sys/class/misc/nitro_enclaves/pcie_slots`. Each step takes 128 MiB of the pool.
  - CPU overcommit allows up to 5× the parent's vCPUs. Dedicated vCPUs come in whole cores from the pool, and the first core stays with the parent.
  - On two-thread cores, real compute is about half the vCPUs.
  - Up to 32 devices per sandbox.
  - No GPU, no snapshot or restore, no pause, no resize, and no hibernation of the parent.
- **Modes.** Chosen at launch:
  - *Confidential* (the default): the parent cannot read the sandbox's memory, and an NSM gives attestation.
  - *Direct*: the parent shares the memory, with no attestation, like a nested VM, and has more throughput.
- **Networking.** A TAP device on the parent, which the parent NATs. A sandbox has no IMDS of its own. Unless the parent rejects `169.254.169.254` from the TAP, the sandbox reads the parent's role credentials. vsock: the guest dials CID 2 port N and reaches `<uds_path>_N` on the parent; the parent dials the guest with `CONNECT N` on `uds_path`.
- **Storage.** Raw image files, instance store or EBS volumes, attached as virtio-blk. The parent can read every disk it presents.
- **Attestation:**
  - PCR0 covers the image (kernel, cmdline, initrds and launch flags), PCR3 the parent's role ARN, PCR4 its instance ID, and PCR5 the PCI devices.
  - KMS `Decrypt`/`GenerateDataKey` with `--recipient` answers `CiphertextForRecipient`, encrypted to the sandbox's own key, and only if the key policy's `kms:RecipientAttestation:PCR*` conditions hold.
  - Debug mode zeroes PCR0/1/2/5/6, so a policy must check PCR0.
  - Expected PCRs come from `eif_build`.
- **EKS / ECS.** EKS: `runtimeClassName: isolated-sandbox` or `confidential-sandbox` (Kubernetes 1.36+, US regions for now), with a Cedar policy per pod. ECS: Managed Instances only. Both need a cluster; Backplane uses neither (below).
- **Startup latency is not published.** `bp-nitro up` reports `boot_ms` so the first live run measures it.
- **Price.** No charge beyond the instance and its disks.

## Architecture

```
 hub (this machine)                         EC2 parent (the person's, one for many sandboxes)
 sandbox::Nitro ──aws ec2 describe/start/stop──▶ (the instance)
        │
        └──ssh──▶ sudo bp-nitro status|list|up|wait|down|run|pipe|prune|secret
                     │ tap bpnN + nft table bp_nitro_N (NAT out; no IMDS, no other sandbox, nothing on the parent; conntrack cap)
                     │ firecracker --config-file cfg.json (made by the hub)        ┌───────── Nitro Sandbox N ─────────┐
                     └─ bp-vsock.py ── vsock 5000 ─────────────────────────────────▶ bp-guest.py (user bp)             │
                                                                                    │ vda: rootfs.ext4 (shared, ro)    │
                                                                                    │ vdb: work.img (src/ target/ ...) │
                                                                                    └──────────────────────────────────┘
```

- **The backend interface** is `sandbox::Backend`, five steps that Boat and Nitro both do: `up(spec)`, `sync_in(root, list)`, `run(cwd, argv, env)` (the command's own exit code), `pull(remote, list, local)` and `down`. `ready()` says why a backend cannot be used. `sandbox::order` picks backends from the settings: `local` first, then Boat (builds only), then Nitro. Agent threads and the hub only go to Nitro with their own switches (`sandbox.nitro.threads`, `sandbox.nitro.hub`).
- **The hub makes every decision.** It checks the parent's pools (`fits`: the limits above), picks the sandbox number, and writes the Firecracker config and the nft rules. The parent only writes them and starts Firecracker. The same rules apply wherever the job comes from.
- **Why EC2 and not EKS/ECS.** A cluster costs money and runs all the time, and a pod spec does not carry a worktree. One parent with Firecracker is the whole thing.
- **The parent's lifecycle.** The backend starts a stopped parent when a job needs it; its disk keeps the images and the warm `target/`. `stop_if_idle` stops it after `sandbox.nitro.idle` seconds (900) with no sandbox. Without an instance ID (an SSH-only parent the person keeps on, e.g. on the tailnet), AWS is never called.
- **Plain sandboxes** (builds, risky commands) boot `vmlinuz` + `initrd.img` with CPU overcommit. **Attested sandboxes** (agent threads with secrets) boot `guest.eif` with dedicated cores, the documented attested path.

## Files in and out

These are the same as Boat (`docs/BOAT-BUILD.md`):

- **In.** `git ls-files -z --cached --others --exclude-standard` is the list. The list goes to `/work/.bp/list`, then `rsync -e scripts/nitro-ssh.sh` sends those files into `/work/src` (by checksum, mtimes not kept). `bp-nitro prune` (boat-prune.py inside the guest) deletes what the list does not name. `.git`, ignored files and the home never go.
- **The channel.** `nitro-ssh.sh` runs `ssh parent sudo bp-nitro pipe NAME rsync --server ...`. `bp-vsock.py` dials the guest's port 5000, and `bp-guest.py` runs the rsync server there as user `bp`.
- **Run.** `bp-nitro run NAME CWD ARGV ENV` (base64, `-` for empty) returns framed stdout and stderr, kept apart, then the exit code. The folder is always inside `src/`. Only build knobs pass into the environment, never a name with TOKEN, KEY, SECRET, PASS, CRED, AUTH, COOKIE or SESSION (`sandbox::env_allowed`, the same rule as `bo_env`). Exit code 255 from ssh or 97 from the vsock is the transport, never a test result.
- **Out.** rsync back through the same pipe, from a list (like `boat-artifacts.py`). The caller checks hashes as `bo_pull` does.
- **Disks.** The work disk stays between jobs (`down` keeps it; `--purge` deletes it), so `target/` and the cargo cache stay warm. The system disk is one read-only image for every sandbox, with an overlay per sandbox on its work disk.

## How Backplane's MCP tools reach a sandbox (agent threads; designed, not built)

There are two shapes, in order:

1. **Risky tool calls only.** The agent CLI stays on the hub with its login. Its shell tool runs through `Backend::run` in the thread's sandbox (`bp-thread-<id>`), so the code it writes runs isolated. The MCP tools stay on the hub as now. No secret leaves this machine. This is the first step: it needs only the shell tool's runner to call the backend when `sandbox.nitro.threads` is on.
2. **The whole agent in the sandbox.** The agent and Backplane's MCP bridge run in the guest. The bridge dials vsock CID 2 port 5100. Firecracker forwards that to `/var/lib/bp-nitro/<name>/v.sock_5100` on the parent, and the hub keeps an `ssh -R <that socket>:127.0.0.1:<hub port>` open for that sandbox's life. The bridge gets a token minted for that one thread, which the hub revokes at `down`. The guest has no other route to the hub: the TAP's input chain refuses everything on the parent.

## Secrets: attestation, not copies

- **Builds get none.** Nothing of the person's goes to a box, as with Boat.
- **What an agent in a sandbox needs** (its provider login, an API key) is released only by KMS, and only to a sandbox running Backplane's measured image:
  1. Once, where the person is: `aws kms encrypt` turns the value into a ciphertext. The ciphertext is not secret, and it can sit on the parent.
  2. The key's policy lets the parent's role decrypt only when `kms:RecipientAttestation:PCR0` is the image's PCR0 (from `guest-build.sh`'s `pcrs.json`) and `PCR3` is the role. `nitro::kms_statement` makes it and refuses an empty or zero PCR0.
  3. The guest's `/init` makes an RSA key, gets an attestation document carrying the public key from `/dev/nsm`, and sends it to the parent on vsock 5005. `bp-nitro secret` calls `kms decrypt --recipient` and returns the answer on 5006. Only the guest can decrypt it, into `/run/bp/secret` (tmpfs).
  4. The parent relays bytes and never has the value. Neither the hub nor the vault copies it into the box.
- **The system disk is not measured.** The attested initrd holds its SHA-256 and refuses to boot from any other.

## Cost model

One parent, many sandboxes. AWS charges for the instance and its EBS volume, nothing per sandbox. On-demand, us-east-1, checked 2026-11-10:

| parent | $/h | vCPU / GiB | what fits at once |
|---|---|---|---|
| c7i.xlarge | 0.1785 | 4 / 8 | a smoke test: two 2-vCPU / 2 GiB sandboxes |
| c7i.2xlarge | 0.357 | 8 / 16 | one 8-vCPU / 12 GiB build, or 6 small sandboxes |
| c7i.4xlarge | 0.714 | 16 / 32 | one 16 GiB build + four 2 GiB agent sandboxes |
| c7g.4xlarge | 0.58 | 16 / 32 | the same on Graviton: one thread per core, but arm64 (not for this desktop's builds) |

- **Example.** c7i.4xlarge, 8 hours a day: $171/month plus 200 GB gp3 ($16) = **$187/month**. With four sandboxes sharing it, that is $0.18 per sandbox-hour.
- **Stopped,** it costs only its disk.
- **The idle stop** (900 s) and starting it only when a job needs it keep the hours down.
- **`sandbox.status`** shows the plan and this estimate. `{probe:true}` asks AWS for the parent's state (describe only).

## What is built, and what is not

- **Built:**
  - The trait.
  - Boat and Nitro backends.
  - The limit checks, configs, firewall, KMS policy, PCR math, cost, plan, parent start/stop, and the RPC.
  - The parent's `bp-nitro`, the vsock agent and helper, the guest init, the image build, the parent's user data and push.
  - Tests with a fake AWS, and the protocol loop.
- **Not yet:**
  - `scripts/cargo-q` does not offload to Nitro. It needs the hub's config maker from the scripts: a `backplane-hub sandbox` subcommand.
  - The shell-tool runner for agent threads (shape 1 above).
  - The MCP relay (shape 2).
  - A client page for the settings.
  - Nothing has run on a real parent: the guest image, the boot, `boot_ms`, and the DMB paths in confidential mode are all unproven until the first live run.

## Settings

| key | default | meaning |
|---|---|---|
| `sandbox.nitro` | off | `on` to use Nitro at all |
| `sandbox.nitro.threads` | off | agent threads may use it |
| `sandbox.nitro.hub` | off | the hub or bots may use it |
| `sandbox.nitro.region` | us-east-1 | |
| `sandbox.nitro.profile` | | the person's AWS CLI profile; credentials stay in the CLI's own files |
| `sandbox.nitro.instance` | | the parent, `i-…` |
| `sandbox.nitro.ssh` | | `ec2-user@host`; without it, the instance's public DNS name is used |
| `sandbox.nitro.type` | c7i.4xlarge | used for the plan and the cost |
| `sandbox.nitro.disk` | 200 | GiB of gp3, used for the plan and the cost |
| `sandbox.nitro.idle` | 900 | seconds before the parent is stopped |
| `sandbox.nitro.kms` | | the key ARN for attested secrets |
| `sandbox.nitro.mode` | confidential | or `direct` |
| `sandbox.boat` | | `off` leaves Boat out of the order |

## The first live run

1. The person approves an instance type, region and hourly cost. They provide an AWS CLI profile, an EC2 key pair, a security group open on port 22 to them, and an instance profile `bp-nitro-host`. The role needs no policy for builds; attestation adds `kms:Decrypt` on the one key.
2. They run `nitro::plan`'s `aws ec2 run-instances …` with `scripts/nitro/host-setup.sh` as user data. It reboots once to reserve the pool.
3. `scripts/nitro/host-push.sh ec2-user@<host>`: installs `bp-nitro` and builds the images.
4. Set `sandbox.nitro=on` and `sandbox.nitro.instance=i-…`. Then `sandbox.status {probe:true}`, then one `up` → `run` → `down` of a small sandbox, noting `boot_ms`.
5. Attestation: on the parent, run `sudo bash ~/bp-nitro-src/scripts/nitro/host-setup.sh --attest` (it builds eif_build and nsm-attest), then `guest-build.sh` again, which prints PCR0. Make a KMS key with the `kms_statement` policy, then `up` an attested sandbox and run `secret`.
6. Afterwards: stop the parent, or terminate it and delete its volume.
