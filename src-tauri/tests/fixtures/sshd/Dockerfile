# Minimal SSH server for RemoteOpsX integration tests.
# Built from a base image so the test never depends on pulling a prebuilt
# SSH image from Docker Hub. Installs the exact tools the health probe uses
# (ss, ps, df) so the live path is genuinely exercised.
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        openssh-server iproute2 procps coreutils ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir -p /run/sshd

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 2222
ENTRYPOINT ["/entrypoint.sh"]
