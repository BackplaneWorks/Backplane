FROM ubuntu:26.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq && apt-get install -y -qq xvfb i3 ibus xdotool xclip python3-xlib x11-utils x11-xkb-utils x11-xserver-utils imagemagick dbus dbus-user-session ffmpeg mesa-vulkan-drivers libvulkan1 \
  libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libxcomposite1 libxdamage1 libxrandr2 libgbm1 libasound2t64 libpango-1.0-0 libcairo2 libxkbcommon-x11-0 libxcb-xkb1 libfontconfig1 libxres1 locales sudo git ca-certificates fonts-dejavu-core >/dev/null && useradd -m u && echo 'u ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/u
