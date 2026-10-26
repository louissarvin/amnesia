# syntax=docker/dockerfile:1
# Build from the repo root: docker build -f infra/images/tor.Dockerfile .

ARG RUNTIME_IMAGE=debian:trixie-20261020-slim

FROM ${RUNTIME_IMAGE}
# Empty means whatever trixie ships (0.4.9.x); set e.g. 0.4.9.14-0+deb13u1 for a reproducible pin.
ARG TOR_VERSION=
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates "tor${TOR_VERSION:+=$TOR_VERSION}" \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 10004 amnesia-tor \
    && useradd -u 10004 -g 10004 -M -d /nonexistent -s /usr/sbin/nologin amnesia-tor \
    && install -d -o 10004 -g 10004 -m 0700 /var/lib/tor
# Baked in so the torrc is covered by the image digest.
COPY infra/vps/torrc /etc/tor/torrc
USER 10004:10004
ENTRYPOINT ["/usr/bin/tor", "-f", "/etc/tor/torrc"]
