# TEST-ONLY PRIVILEGED-PATH E2E IMAGE (#2904)
#
# WARNING: This image contains NOPASSWD sudo rules, a synthetic password for a
# test user, and fake pkexec/askpass shims. It is STRICTLY for unattended E2E
# and Harbor verification. NEVER use it as a production session image and NEVER
# install these shims or sudoers rules on a host machine.

FROM python:3.12-slim-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash coreutils sudo procps ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Dedicated non-root test user with a synthetic password for sudo askpass tests
# (B-1 correct password, B-2 wrong password, S-2 persistent shell sudo, K-2 setup-wizard).
RUN useradd -m -s /bin/bash e2etest \
    && echo 'e2etest:e2e-sudo-fixture' | chpasswd \
    && usermod -aG sudo e2etest \
    && printf 'e2etest ALL=(ALL) NOPASSWD: /usr/local/bin/e2e-nopasswd-probe\n' \
        > /etc/sudoers.d/99-e2e-test-only \
    && chmod 0440 /etc/sudoers.d/99-e2e-test-only \
    && visudo -cf /etc/sudoers.d/99-e2e-test-only

COPY bin/pkexec /usr/local/bin/pkexec
COPY bin/e2e-askpass /usr/local/bin/e2e-askpass
COPY bin/e2e-nopasswd-probe /usr/local/bin/e2e-nopasswd-probe
RUN chmod 0755 /usr/local/bin/pkexec /usr/local/bin/e2e-askpass /usr/local/bin/e2e-nopasswd-probe

RUN mkdir -p /app /tmp/e2e-privileged-logs \
    && chown -R e2etest:e2etest /app /tmp/e2e-privileged-logs

ENV LIBRAGENT_E2E_PRIVILEGED_LOG_DIR=/tmp/e2e-privileged-logs \
    LIBRAGENT_E2E_FAKE_PKEXEC_MODE=fail \
    LIBRAGENT_E2E_FAKE_ASKPASS_MODE=succeed

USER e2etest
WORKDIR /app

CMD ["sleep", "infinity"]
