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

## Bake the image ONCE (x86 — the author's Mac is ARM, can't build this)

```bash
# Pin the commit so the baked guest/image_id is reproducible.
docker build --build-arg GIT_SHA=$(git rev-parse HEAD) \
  -t <dockerhub-user>/sincerin-prover:n8 worker/
docker push <dockerhub-user>/sincerin-prover:n8
```

The image bakes PARADA-1 steps 1–7 (deps · rust · rzup+risc0-groth16 · cuda
feature · **sppark 0.1.12 pin** · `host` built with the guest prebuilt). Runtime =
just `host prove`. The build is heavy (compiles risc0 + CUDA kernels + guest);
expect a multi-GB image and a 15–30 min build.

## RunPod serverless endpoint — REQUIRED guardrails (confirm BEFORE the first real job)

Create a Serverless endpoint from the pushed image with, explicitly:

- [ ] **GPU: RTX 3090** (Ampere, sm_86 — the PARADA-1 hardware; the 5m04s number
      holds). NOT Blackwell — the handler's startup gate refuses it anyway.
- [ ] **Min workers = 0** → **$0 when idle** (scale-to-zero). No always-on worker.
- [ ] **Max workers = 1–2** → a runaway can't fan out.
- [ ] **Execution timeout ≈ 900 s (15 min)** per job → a hung prove is killed, not
      billed forever (a real N=8 prove is ~5 min; 15 min is generous headroom).
- [ ] **Spending limit / billing alert** set on the RunPod account.

The sequencer reads `RUNPOD_ENDPOINT_ID` + `RUNPOD_API_KEY` from env (and optional
`RUNPOD_BASE_URL`). Never commit the API key.

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
