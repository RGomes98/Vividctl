# Ubuntu 24.04 with libdbus-1-dev, to build the `dbus` crate against the system libdbus.
FROM sp06-gui
RUN apt-get update && apt-get install -y --no-install-recommends libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*
