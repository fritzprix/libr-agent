# LibrAgent default Docker session image.
#
# Contents: bash, git, node (npm, npx), python3, uv (uvx).
# Not included: docker CLI, compilers, build tools.
# Compressed size target: 200 MB or less per architecture
# (.github/workflows/session-image.yml fails the build above it).
FROM ghcr.io/astral-sh/uv:0.13.6 AS uv

FROM node:26-trixie-slim

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
