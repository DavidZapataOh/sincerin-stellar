# Sincerin GPU prover — RunPod Serverless worker image.
#
# Bakes the HARDENED PARADA-1 recipe (scripts/gpu_latency_check_n8.sh, commit
# e807be1) as build layers, so at RUNTIME the only work is `host prove` (~5 min on
# an RTX 3090). NO Docker daemon at runtime: the STARK→Groth16 wrap runs NATIVE
# CUDA (risc0-groth16/cuda → rapidsnark FFI), which is exactly why a plain CUDA
# *devel* container suffices.
#
# Build ONCE (on x86 — RunPod's image build or any x86 box; the author's Mac is
# ARM and cannot build this):
#   docker build --build-arg GIT_SHA=<sha> -t <user>/sincerin-prover:n8 worker/
#
# Target GPU: Ampere/Ada (RTX 3090 — the PARADA-1 hardware, 5m04s). NEVER
# Blackwell (sm_100/sm_120): risc0 3.0.5 + sppark 0.1.12 + CUDA 12.x don't support
# it — the handler's startup gate refuses it before any billable work.

FROM nvidia/cuda:12.4.1-devel-ubuntu22.04

ARG GIT_SHA=main
ARG R0_VERSION=3.0.5
ENV DEBIAN_FRONTEND=noninteractive \
    ROLLUP_LOCAL_GUEST=1 \
    RISC0_DEV_MODE=0 \
    PATH=/root/.cargo/bin:/root/.risc0/bin:/usr/local/cuda/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    LD_LIBRARY_PATH=/usr/local/cuda/lib64 \
    LIBCLANG_PATH=/usr/lib/llvm-14/lib

# ── 1) system deps: protoc + clang/libclang/llvm (bindgen) + python (handler) ──
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential libssl-dev pkg-config curl git ca-certificates \
      protobuf-compiler clang libclang-dev llvm-dev \
      python3 python3-pip \
 && rm -rf /var/lib/apt/lists/* \
 && command -v protoc \
 && ls "$LIBCLANG_PATH"/libclang*.so* >/dev/null \
      || { echo "ERROR: no libclang at LIBCLANG_PATH=$LIBCLANG_PATH (base image llvm changed?)"; exit 1; }

# ── 2) rust stable ────────────────────────────────────────────────────────────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
 && cargo --version

# ── 3) RISC Zero toolchain: guest-local rust + cargo-risczero + r0vm + the ─────
#       Groth16 wrap component (risc0-groth16). Without it the native wrap can't
#       find its circuit/proving key.
RUN curl -L https://risczero.com/install | bash \
 && rzup install rust \
 && rzup install cargo-risczero ${R0_VERSION} \
 && rzup install r0vm ${R0_VERSION} \
 && rzup install risc0-groth16

# ── 4) the repo at a pinned commit (reproducible image) ────────────────────────
RUN git clone https://github.com/DavidZapataOh/sincerin-stellar.git /sincerin
WORKDIR /sincerin
RUN git fetch origin && git checkout ${GIT_SHA} && git log --oneline -1

# ── 5) enable the risc0-zkvm `cuda` feature for the native wrap ────────────────
RUN sed -i 's#risc0-zkvm\( *\)= { workspace = true }#risc0-zkvm\1= { workspace = true, features = ["cuda"] }#' host/Cargo.toml \
 && grep -q 'features = \["cuda"\]' host/Cargo.toml

# ── 6) pin the CUDA stack risc0 3.0.5 was TESTED with ──────────────────────────
#       Fresh resolution pulls sppark 0.1.15 → "illegal memory access" at runtime.
#       risc0 v3.0.5 lock = sppark 0.1.12 / blst 0.3.15.
RUN cargo fetch \
 && cargo update -p sppark --precise 0.1.12 \
 && cargo update -p blst --precise 0.3.15 \
 && grep -A1 'name = "sppark"' Cargo.lock | grep -q '"0.1.12"'

# ── 7) build host (CUDA kernels + guest LOCAL, no Docker) — baked in ───────────
RUN cargo build --release -p host \
 && test -x target/release/host

# ── 8) the RunPod serverless handler ──────────────────────────────────────────
RUN pip3 install --no-cache-dir runpod
COPY handler.py /sincerin/handler.py
CMD ["python3", "-u", "/sincerin/handler.py"]
