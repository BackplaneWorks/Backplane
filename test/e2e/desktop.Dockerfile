# The desktop's userland (Ubuntu 26.04, glibc 2.43) on a Boat box (24.04): dev builds of the hub and client are linked against it.
# test/e2e/boat-inner.sh builds this and runs its mode inside when the shipped client needs a newer glibc than the box has.
FROM ubuntu:26.04
ARG UID=1000
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq && apt-get install -y -qq xvfb i3 ibus ibus-hangul fcitx5 fcitx5-hangul xdotool xclip python3-xlib python3-pip x11-utils x11-xkb-utils x11-xserver-utils imagemagick dbus dbus-user-session ffmpeg mesa-vulkan-drivers libvulkan1 \
  libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libxcomposite1 libxdamage1 libxrandr2 libgbm1 libasound2t64 libpango-1.0-0 libcairo2 libxkbcommon-x11-0 libxcb-xkb1 libfontconfig1 libxres1 \
  locales sudo git curl ca-certificates fonts-dejavu-core >/dev/null \
 && (userdel -r ubuntu 2>/dev/null || true) && useradd -m -u "$UID" u && echo 'u ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/u
