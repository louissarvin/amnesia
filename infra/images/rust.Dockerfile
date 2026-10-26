# syntax=docker/dockerfile:1
# Build from the repo root: docker build -f infra/images/rust.Dockerfile --target desk .
# Base tags are pinned here; infra/digests.json records the digests they resolved to.

ARG RUST_IMAGE=rust:1.93.1-slim-trixie
ARG RUNTIME_IMAGE=debian:trixie-20261020-slim

FROM ${RUST_IMAGE} AS builder
ARG TARGETARCH
# 4 keeps Docker Desktop's 4 GB VM from OOMing.
ARG CARGO_BUILD_JOBS=4
# Overrides rust-toolchain.toml so rustup does not fetch clippy and rustfmt into the image.
ENV RUSTUP_TOOLCHAIN=1.93.1 \
    CARGO_TARGET_DIR=/build/target \
    CARGO_TERM_COLOR=never \
    CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
WORKDIR /src
# Offline build: sources come from `cargo vendor --locked vendor/ > infra/cargo-vendor.toml`
# on the host. Its relative `vendor/` resolves against /src, the parent of /src/.cargo.
COPY Cargo.toml Cargo.lock ./
COPY vendor vendor
COPY infra/cargo-vendor.toml .cargo/config.toml
COPY crates crates
# The target dir lives in a cache mount, so binaries are copied out in the same step.
RUN --network=none \
    --mount=type=cache,id=amnesia-target-${TARGETARCH},target=/build/target \
    cargo build --release --locked --offline -p amnesia-desk -p amnesia-lab \
    && install -d /out \
    && install -m 0755 /build/target/release/amnesia-desk /build/target/release/amnesia-lab /build/target/release/seize /out/

FROM ${RUNTIME_IMAGE} AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 10001 amnesia \
    && useradd -u 10001 -g 10001 -M -d /nonexistent -s /usr/sbin/nologin amnesia

FROM runtime AS desk
# Empty named volumes inherit this ownership and mode on first mount.
RUN install -d -o 10001 -g 10001 -m 0700 /var/lib/amnesia/desk /run/amnesia/epochs
COPY --from=builder /out/amnesia-desk /usr/local/bin/
ENV AMNESIA_ROOT=/var/lib/amnesia/desk \
    AMNESIA_EPOCHS_DIR=/run/amnesia/epochs
USER 10001:10001
WORKDIR /var/lib/amnesia/desk
ENTRYPOINT ["/usr/local/bin/amnesia-desk"]

FROM runtime AS lab
# curl for the faucet claim, tar for seizure bundles. The desk stage never gets these.
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl tar \
    && rm -rf /var/lib/apt/lists/* \
    && install -d -m 0700 /var/lib/amnesia/lab
COPY --from=builder /out/amnesia-lab /out/seize /usr/local/bin/
ENV AMNESIA_LAB_ROOT=/var/lib/amnesia/lab
# The desk's uid, so it reads the desk's 0700 files without capabilities. A fresh lab_root
# volume inherits this owner. Only compose.ramproof.yaml runs it as root.
RUN chown 10001:10001 /var/lib/amnesia/lab
USER 10001:10001
# The lab CLI resolves some paths (var/lab-arti) relative to the working dir.
WORKDIR /var/lib/amnesia/lab
# No ENTRYPOINT so `compose run lab amnesia-lab init-all` and `seize` work as written.
CMD ["/usr/local/bin/amnesia-lab", "serve"]
