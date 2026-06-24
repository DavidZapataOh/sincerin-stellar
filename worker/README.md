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

## Bake on a GPU VM (~$0.50; CI/CPU can't — risc0 compiles CUDA with `-arch=native`)

Two reasons the bake needs a **real GPU VM** (not CI, not a RunPod pod):
1. Producing cbeab7aa needs the `r0.1.88.0` Docker guest build → a **real Docker daemon**
   (a RunPod pod is a container with none).
2. risc0's CUDA kernels compile with **`-arch=native`**, which needs an **Ampere/Ada GPU
   PRESENT** (a CPU CI runner falls back to an unsupported arch → build fails). The VM
   replicates the PARADA-1 box where the build already passes — no patching.

So the bake **does** touch a GPU (~$0.50, bounded) — only to compile kernels, not to
prove. **Recommended VM: AWS `g5.xlarge`** (A10G, sm_86 — same arch class as the 3090),
**Ubuntu 22.04 Deep Learning Base GPU AMI** (Docker + CUDA + driver + nvidia-container-
toolkit preinstalled), ≥60 GB disk. Billed per-second while running; **TERMINATE right
after the push.**

```bash
# 0. ssh into the VM, then SANITY CHECK first (aborts in seconds if Docker/GPU are wrong):
curl -fsSL https://raw.githubusercontent.com/DavidZapataOh/sincerin-stellar/sdd/s3-05/scripts/vm_bake_sanity.sh | bash

# 1. clone the branch
git clone -b sdd/s3-05 https://github.com/DavidZapataOh/sincerin-stellar.git && cd sincerin-stellar

# 2. STAGE 1 — build the production host. Verifies image_id == cbeab7aa with `host execute`
#    and ABORTS if it differs → a wrong-guest image can never be shipped.
bash worker/build-host.sh                                    # → worker/dist/{host,risc0-home}

# 3. STAGE 2 — slim runtime image (CUDA runtime + the host + groth16 artifacts + handler;
#    NO toolchain, NO Docker at runtime). Push to GHCR (needs a GitHub PAT, scope write:packages).
docker build -t ghcr.io/davidzapataoh/sincerin-prover:n8 worker/
echo "$GHCR_PAT" | docker login ghcr.io -u davidzapataoh --password-stdin
docker push ghcr.io/davidzapataoh/sincerin-prover:n8

# 4. TERMINATE the VM NOW (AWS console → Instances → Terminate) → $0 after.
```

After the first push, make the GHCR package **public** (GitHub → Packages →
`sincerin-prover` → settings → visibility: public) so RunPod pulls it without creds.

## RunPod serverless endpoint — REQUIRED guardrails (confirm BEFORE the first real job)

> **GPU selection is by VRAM CATEGORY, not exact model** (RunPod docs). The 3090
> lives in the category **"L4, A5000, 3090 (24 GB)"** — selecting it means the
> worker may run on L4, A5000, OR RTX 3090. All three are Ampere/Ada, CUDA-12.4
> compatible, **NONE is Blackwell** (B200 is its own unselected category). Select
> ONLY this one category → no fallback to others → the worker can NEVER touch
> Blackwell. The handler's runtime gate is the final backstop.

Create a Serverless endpoint from the pushed image with, explicitly:

- [ ] **GPU category: ONLY "L4, A5000, 3090 (24 GB)"** — no other category selected
      (so there is no fallback to a non-compatible GPU). The actual GPU per job is
      reported in the output (`gpu`); a 3090 is the direct PARADA-1 comparison.
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
