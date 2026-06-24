#!/usr/bin/env python3
"""
Sincerin GPU prover — RunPod Serverless handler.

Per job: receive the inputs-file JSON (the SAME shape the sequencer's LocalProver
writes and golden/*_inputs.json use), run the REAL `host prove`
(RISC0_DEV_MODE=0, native CUDA), and return {seal_hex, image_id_hex, journal_hex}.

CERO-MOCKS: it refuses to return a dev-mode seal (selector ffffffff). A non-zero
`host prove` exit becomes an error (→ the sequencer marks the batch Failed, the
note's lock is released, re-submit possible). The startup gate refuses an
incompatible GPU (Blackwell) BEFORE any billable proving.
"""

import json
import os
import pathlib
import subprocess
import tempfile

import runpod

HOST_BIN = "/sincerin/target/release/host"


def _startup_gate() -> None:
    """Reject Blackwell (sm_100/sm_120, CC>=10) and pre-Volta (<sm_70) GPUs early."""
    try:
        out = subprocess.run(
            ["nvidia-smi", "--query-gpu=name,compute_cap", "--format=csv,noheader"],
            capture_output=True, text=True, timeout=20,
        ).stdout.strip().splitlines()[0]
        name, cc = [p.strip() for p in out.split(",")][:2]
        major = int(cc.split(".")[0])
        sm = "sm_" + cc.replace(".", "")
        if major >= 10:
            raise RuntimeError(
                f"GPU {name} is Blackwell {sm} (CC {cc}); risc0 3.0.5 + sppark 0.1.12 + "
                "CUDA 12.x do NOT support it. Use Ampere/Ada: RTX 3090/4090, A40, A100, L4."
            )
        if major < 7:
            raise RuntimeError(f"GPU {name} too old ({sm}, CC {cc} < sm_70).")
        print(f"[gate] OK: {name} {sm} (CC {cc})", flush=True)
    except RuntimeError:
        raise
    except Exception as e:  # nvidia-smi missing/odd output — warn, don't hard-fail
        print(f"[gate] WARN: could not read compute_cap ({e}); do NOT schedule Blackwell.", flush=True)


def handler(job):
    inp = (job or {}).get("input")
    if not isinstance(inp, dict):
        return {"error": "job.input must be the inputs-file JSON object"}

    with tempfile.TemporaryDirectory() as tmp:
        d = pathlib.Path(tmp)
        inputs_path = d / "inputs.json"
        out_dir = d / "out"
        out_dir.mkdir()
        inputs_path.write_text(json.dumps(inp))

        env = dict(os.environ, RISC0_DEV_MODE="0", ROLLUP_LOCAL_GUEST="1")
        proc = subprocess.run(
            [HOST_BIN, "prove", "--inputs", str(inputs_path), "--out", str(out_dir)],
            capture_output=True, text=True, env=env,
        )
        if proc.returncode != 0:
            return {"error": f"host prove failed (exit {proc.returncode}): {proc.stderr[-800:]}"}

        seal_path = out_dir / "seal.hex"
        if not seal_path.exists():
            return {"error": "host prove produced no seal.hex"}
        seal_hex = seal_path.read_text().strip()
        if seal_hex[:8].lower() == "ffffffff":
            return {"error": "dev-mode seal (ffffffff) — REAL Groth16 proofs only; refusing to return"}

        image_id_hex = (out_dir / "image_id.hex").read_text().strip()
        journal_hex = (out_dir / "journal.bin").read_bytes().hex()
        return {"seal_hex": seal_hex, "image_id_hex": image_id_hex, "journal_hex": journal_hex}


_startup_gate()
runpod.serverless.start({"handler": handler})
