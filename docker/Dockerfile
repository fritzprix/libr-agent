# LibrAgent default Docker session image.
#
# Contents: bash, git, node (npm, npx), python3, uv (uvx).
# Not included: docker CLI, compilers, build tools.
# Compressed size limit per architecture: SIZE_LIMIT_MB in
# .github/workflows/session-image.yml (the build fails above it).
# Base images are pinned by digest; update tag and digest together.
FROM ghcr.io/astral-sh/uv:0.13.6@sha256:af22a52f5eafef3d78e551599d405c02df61919d18028f0cea8eda93bc9986d0 AS uv

FROM node:26-trixie-slim@sha256:e4db4c914bf8ad241bd24804fc25d272d89bad97e68cdc8a1414862790a02c39

LABEL org.opencontainers.image.source=https://github.com/fritzprix/libr-agent

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        git \
        python3 \
    && rm -rf /var/lib/apt/lists/* \
    && ln -sf /usr/bin/python3 /usr/local/bin/python

COPY --from=uv /uv /uvx /usr/local/bin/

RUN mkdir -p /workspace /skills
WORKDIR /workspace

RUN bash --version >/dev/null \
    && git --version \
    && node --version \
    && npm --version \
    && npx --version \
    && python3 --version \
    && uv --version \
    && uvx --version

CMD ["tail", "-f", "/dev/null"]
