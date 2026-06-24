# Sincerin GPU prover — RunPod Serverless worker image (STAGE 2, runtime).
#
# Slim runtime: it COPYs the PRODUCTION host binary built by `worker/build-host.sh`
# (STAGE 1), which has the contract-bound image_id **cbeab7aa…0d46** baked in (the
# reproducible r0.1.88.0 Docker guest build — NOT ROLLUP_LOCAL_GUEST). No rust
# toolchain, no source, no guest rebuild, NO Docker at runtime: the STARK→Groth16
# wrap runs native CUDA (risc0-groth16/cuda).
#
# Why two stages: producing cbeab7aa REQUIRES the r0.1.88.0 Docker guest build,
# which needs a Docker daemon — available only at BAKE time on an x86 box (Stage 1
# mounts the docker socket). The runtime image needs no Docker. A worker baked
# with ROLLUP_LOCAL_GUEST would get a different image_id and have EVERY settle
# rejected on-chain — hence Stage 1 verifies cbeab7aa before this image is built.
#
# Bake (on an x86 box WITH Docker):
#   GIT_SHA=<sdd/s3-05 sha> bash worker/build-host.sh      # → worker/dist/{host,risc0-home}
#   docker build -t <user>/sincerin-prover:n8 worker/
#   docker push  <user>/sincerin-prover:n8
#
# Same nvidia/cuda:12.4.1-devel base as Stage 1 → the host binary is ABI-compatible.
# Target GPU: the "L4, A5000, 3090 (24 GB)" category (all Ampere/Ada, CUDA 12.4,
# never Blackwell). The handler's startup gate refuses an incompatible GPU.

FROM nvidia/cuda:12.4.1-devel-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive \
    RISC0_DEV_MODE=0 \
    RISC0_HOME=/root/.risc0 \
    PATH=/usr/local/cuda/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    LD_LIBRARY_PATH=/usr/local/cuda/lib64

# runtime deps only (the host links libssl/CUDA; python runs the handler)
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libssl3 python3 python3-pip \
 && rm -rf /var/lib/apt/lists/* \
 && pip3 install --no-cache-dir runpod

# the prebuilt PRODUCTION host (cbeab7aa) + its risc0 groth16 proving artifacts,
# both from worker/build-host.sh (Stage 1). The image_id is baked into the binary.
COPY dist/host /sincerin/host
COPY dist/risc0-home /root/.risc0
COPY handler.py /sincerin/handler.py

# fail the bake loudly if Stage 1 wasn't run (no binary to ship)
RUN test -x /sincerin/host && test -d /root/.risc0

CMD ["python3", "-u", "/sincerin/handler.py"]
