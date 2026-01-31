# Base image: Debian Bookworm Slim (Lightweight & Compatible)
FROM debian:bookworm-slim

# Prevent interactive prompts during build
ENV DEBIAN_FRONTEND=noninteractive

# Install essential dependencies and Docker CLI prerequisites
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    git \
    unzip \
    gnupg \
    && install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg \
    && chmod a+r /etc/apt/keyrings/docker.gpg \
    && echo \
    "deb [arch="$(dpkg --print-architecture)" signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian \
    "$(. /etc/os-release && echo "$VERSION_CODENAME")" stable" | \
    tee /etc/apt/sources.list.d/docker.list > /dev/null \
    && apt-get update && apt-get install -y --no-install-recommends \
    docker-ce-cli \
    python3 \
    python3-pip \
    python3-venv \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install uv (The Astral Python package manager)
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

# Create aliases/wrappers if necessary (uvx is included with uv)
# Ensure standard generic links exist
RUN ln -sf /usr/bin/python3 /usr/bin/python

# Create workspace and skills directories
WORKDIR /workspace
RUN mkdir -p /skills

# Verify installations
RUN uv --version && \
    node --version && \
    npm --version && \
    docker --version && \
    python --version

# Default command keeps the container running
CMD ["tail", "-f", "/dev/null"]
