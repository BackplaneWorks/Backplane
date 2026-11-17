# The linux-x64 release runner's image (docs/release-runners.md): ubuntu 22.04, so the archive keeps glibc 2.35 as on GitHub's ubuntu-22.04 image,
# with the tools release.yml's build expects that image to have (wget for setup-mold; the libraries libcef.so links against, which GitHub's image has). The runner itself, rustup and every cache live in the volume at /home/runner
# (scripts/ci-linux/entrypoint.sh installs what is missing there), so the image is the same on any machine and holds no credential.
FROM ubuntu:22.04
ARG UID=1000
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      ca-certificates curl git git-lfs sudo python3 python3-venv build-essential pkg-config clang libclang-dev lld perl make file binutils \
      xz-utils zstd unzip zip jq patchelf ninja-build libicu70 tzdata openssh-client rsync gnupg lsb-release wget \
      libnss3 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libcups2 libdbus-1-3 libxdamage1 libxfixes3 libxcomposite1 libxrandr2 libxshmfence1 \
      libpango-1.0-0 libcairo2 libgbm1 libdrm2 libgtk-3-0 libasound2 \
    && rm -rf /var/lib/apt/lists/*
# pinned by sha256 (bump them together with scripts/ci-linux/README)
RUN set -eu; cd /tmp; \
    curl -sSfLo c.tgz https://github.com/Kitware/CMake/releases/download/v3.31.10/cmake-3.31.10-linux-x86_64.tar.gz; \
    echo "3cb3dd247b6a1de2d0f4b20c6fd4326c9024e894cebc9dc8699758887e566ca7  c.tgz" | sha256sum -c -; \
    tar -xzf c.tgz -C /usr/local --strip-components=1; rm c.tgz; \
    curl -sSfLo n.txz https://nodejs.org/dist/v22.23.5/node-v22.23.5-linux-x64.tar.xz; \
    echo "cdeb8670a0c81f51b4dc4ac61919f9b1ad917b304bf8221eb939570663d240f8  n.txz" | sha256sum -c -; \
    tar -xJf n.txz -C /usr/local --strip-components=1 --exclude='*.md' --exclude=LICENSE; rm n.txz; \
    curl -sSfLo s.tgz https://github.com/mozilla/sccache/releases/download/v0.19.0/sccache-v0.19.0-x86_64-unknown-linux-musl.tar.gz; \
    echo "aa73c7270c997894038c0d8f0d297865f57569e1687883322bc431713d4bb9dd  s.tgz" | sha256sum -c -; \
    tar -xzf s.tgz; install -m755 sccache-*/sccache /usr/local/bin/sccache; rm -rf s.tgz sccache-*
# the job user, with sudo as GitHub's image has it (release.yml installs its system libraries with apt)
RUN useradd -m -u $UID -s /bin/bash runner && echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner && chmod 440 /etc/sudoers.d/runner
COPY entrypoint.sh /opt/bp/entrypoint.sh
USER runner
WORKDIR /home/runner
ENTRYPOINT ["/opt/bp/entrypoint.sh"]
