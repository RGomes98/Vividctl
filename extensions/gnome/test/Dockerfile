# Ubuntu with a given GNOME Shell, to run the extension in a headless session.
# 24.04 = Shell 46, 25.04 = 48, 25.10 = 49, 26.04 = 50.
ARG BASE=ubuntu:26.04
FROM ${BASE}

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        gnome-shell dbus-daemon dbus-user-session dconf-gsettings-backend gsettings-desktop-schemas \
        libgl1-mesa-dri libegl1 libglx-mesa0 xdg-desktop-portal-gnome \
        python3 python3-pil ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /tmp
