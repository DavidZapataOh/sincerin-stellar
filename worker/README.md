# Sincerin GPU prover — RunPod Serverless worker

The production proving backend (`PROVER_BACKEND=remote`). The sequencer's
`RemoteProver` POSTs the inputs-file JSON to a RunPod serverless endpoint running
this image; the worker runs the REAL `host prove` (native CUDA, `RISC0_DEV_MODE=0`)
and returns the receipt. **Scale-to-zero: $0 when idle.**

> Nothing here runs without an explicit go. The `RemoteProver` is fully unit-tested
> against a worker-fake (no GPU) — see `cargo test -p sequencer` (`remote_tests`).
> Bake + GPU only after sign-off.

## Wire contract (sequencer ⇄ worker)

- **Request** (`POST /v2/{endpoint}/run`): `{"input": <inputs-file JSON object>}` —
  the exact bytes `inputs_file_json` produces (lock 3, byte-identical to the local
  path and `golden/*_inputs.json`).
- **Response** (`GET /v2/{endpoint}/status/{id}` when `COMPLETED`):
  `{"output": {"seal_hex": "...", "image_id_hex": "...", "journal_hex": "..."}}`.
- The `RemoteProver` reconstructs the `ProvedBatch` through `build_proved_batch`,
  so the **dev-mode (`ffffffff`) and short-seal rejections apply identically** to
  local and remote. The handler ALSO refuses a dev-mode seal at the source.

## CRITICAL: the worker MUST produce image_id `cbeab7aa…0d46`

The on-chain `settle_batch` binds the DEPLOYED guest image_id **cbeab7aa…0d46** (the
reproducible `r0.1.88.0` Docker guest build) and reverts if a receipt's image_id
differs. So the production host MUST be built **WITHOUT** `ROLLUP_LOCAL_GUEST` — that
flag (used only to MEASURE latency in PARADA 1 on a Docker-less box) yields a
different, path-dependent id (`3b0a6d14…`) that the contract would **reject** →
EVERY settle would fail. (See `methods/build.rs:41-48`.) The guest cycles — and so
the ~5min prove time — are identical either way; only the image_id build method
differs.

## Bake on a real GPU VM, with a VALIDATION PROVE before push (~$0.50–1)

The bake needs a **real Docker daemon** (the reproducible `r0.1.88.0` guest build →
cbeab7aa) **and a GPU**: `-arch=native` compiles the CUDA kernels for the VM's GPU
arch (PARADA-1 style, no hack), and — the key safety — `build-host.sh` then runs a
**REAL N=8 prove on the VM's GPU** and refuses to ship unless it verifies. So you
reach Stop A *knowing* the kernels prove, instead of baking blind.

> **Single-arch rule (covers the L4 gap):** native CUDA SASS is per compute-capability
> — an sm_86 build runs on sm_86 GPUs only, NOT sm_89 (L4). So set the serverless
> endpoint to a **single-arch category** matching the bake GPU, and the validation
> prove covers production EXACTLY. Bake on an **sm_86** GPU → endpoint category
> **"A6000, A40 (48 GB)"** (both sm_86). Do NOT use the mixed "L4, A5000, 3090 (24 GB)"
> category — its L4 (sm_89) wouldn't run the sm_86 build, and you can't validate it
> from an sm_86 VM.

GPU-container providers (RunPod, Vast.ai) **don't work** — they're containers with no
Docker daemon. And AWS/GCP/Paperspace gate new-account GPU behind a 1–2 day manual
approval. Use a **real GPU VM with root, self-serve, no approval**: **TensorDock** (KVM
VMs, full root + Docker, instant, $5 min — RTX 3090 ~$0.31/hr or A5000, both sm_86) or
**Trooper.AI** (EU, root-access GPU servers, RTX 3090 ~€0.23/hr, sm_86). Ubuntu 22.04/24.04
(host OS doesn't matter — the build runs in a cuda:12.4.1 container). ≥80 GB disk. Build +
validation prove ~35–50 min → **~$0.20–0.50**. Delete the VM after the push.

```bash
# 0. ssh in. Install Docker + nvidia-container-toolkit if the image lacks them.

# 1. SANITY — Docker daemon + Ampere/Ada GPU + docker --gpus all (aborts in seconds):
curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash

# 2. clone
git clone -b sdd/s3-05 https://github.com/DavidZapataOh/sincerin-stellar.git && cd sincerin-stellar

# 3. STAGE 1 — build (native kernels) + VALIDATION PROVE on the GPU. Verifies image_id
#    == cbeab7aa AND runs a real prove (seal ≠ ffffffff, receipt.verify OK). ABORTS
#    before shipping if anything fails → no broken image reaches GHCR.
bash worker/build-host.sh                                    # → worker/dist/{host,risc0-home}

# 4. STAGE 2 — slim runtime image + push to GHCR (GitHub PAT, scope write:packages).
docker build -t ghcr.io/davidzapataoh/sincerin-prover:n8 worker/
export GHCR_PAT='ghp_...'
echo "$GHCR_PAT" | docker login ghcr.io -u davidzapataoh --password-stdin
docker push ghcr.io/davidzapataoh/sincerin-prover:n8

# 5. DELETE the VM (provider console) → $0 after.
```

After the first push, make the GHCR package **public** (GitHub → Packages →
`sincerin-prover` → settings → visibility: public) so RunPod pulls it without creds.

## RunPod serverless endpoint — REQUIRED guardrails (confirm BEFORE the first real job)

> **GPU selection is by VRAM CATEGORY, not exact model** (RunPod docs), and native
> CUDA SASS is per compute-capability. So the endpoint category MUST be **single-arch
> AND match the arch the host was built+validated on** — otherwise a different-arch
> GPU in a mixed category (e.g. the L4 sm_89 in "L4, A5000, 3090 (24 GB)") can't run
> an sm_86 build, and it reverts at prove time. For an **sm_86** build, the matching
> single-arch category is **"A6000, A40 (48 GB)"** (both sm_86). None is Blackwell;
> the handler's runtime gate is the final backstop.

Create a Serverless endpoint from the pushed image with, explicitly:

- [ ] **GPU category: ONLY a single-arch category matching the bake arch** — for an
      sm_86 build, **"A6000, A40 (48 GB)"** (both sm_86). NOT the mixed "L4, A5000,
      3090 (24 GB)" (its L4 is sm_89). No other category selected (no fallback). The
      validation prove ran on this exact arch, so production is covered.
- [ ] **(if available) Allowed CUDA version ≥ 12.4** — extra host-driver guard.
- [ ] **Active (min) workers = 0** → **$0 when idle** (scale-to-zero). No always-on.
- [ ] **Max workers = 1–2** → a runaway can't fan out.
- [ ] **Execution timeout = 900 s (15 min)** (the default is 600 s; raise it) → a
      hung prove is killed, not billed forever (a real N=8 prove is ~5 min).
- [ ] **Spending limit / billing alert** set on the RunPod account.

The sequencer reads `RUNPOD_ENDPOINT_ID` + `RUNPOD_API_KEY` from env (and optional
`RUNPOD_BASE_URL`). Never commit the API key.

After the endpoint exists and the image is baked, run **one** Stop-A prove and
paste the result:

```bash
RUNPOD_ENDPOINT_ID=xxxx RUNPOD_API_KEY=yyyy bash scripts/stopA_remote_prove.sh
```

It checks: which GPU ran · prove ≈ 5m04s · seal ≠ ffffffff · image_id == the
canonical deployed guest (byte-parity of the baked guest).

## Cost (RTX 3090 serverless, per-second; verify current pricing)

- Idle: **$0** (min-workers 0).
- One N=8 prove ≈ 5 min ≈ **$0.10–0.15**.
- Build + validate (Stop A) + integration proves (Stop B) ≈ a handful of proves +
  a few debug pods ≈ **$2–4**.
- 1-week judging window: ~5 min per judge run; ~**$1–8** depending on traffic.
- **Total s3/05 + s3/03 ≈ $5–15.**

## Stops (no GPU until each is approved)

- **Stop A** — image baked, ONE real prove on the endpoint returns a non-dev-mode
  N=8 receipt with wall-clock ≈ the PARADA-1 5m04s, BEFORE wiring the frontend.
- **Stop B** — full integration gate through the sequencer (`PROVER_BACKEND=remote`):
  happy + collision + the judge's arbitrary recipient + async observed, plus the
  fresh on-chain gate run (a new resolvable settle tx).
