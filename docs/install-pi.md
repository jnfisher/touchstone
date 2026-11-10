# Install on a Raspberry Pi 4

This puts touchstone on a Raspberry Pi 4 (aarch64, Raspberry Pi OS 64-bit) next to a
silvermere/Quicksilver server, reading the game's SQLite database read-only. The same steps work
on any 64-bit Linux box: use the `x86_64-unknown-linux-musl` archive there.

The release binaries are static (musl) and contain the web UI, so there is nothing to install
besides the binary. Commands below run as a user with `sudo`.

## Memory

The Pi 4 has 4 GB, shared with the game. With the stock database (about 27,000 rooms, 1,100
monsters, 1,950 items) a freshly started server uses about 41 MB resident (static musl build,
measured on x86_64 after startup; the glibc build measured 49 MB). A live silvermere database with
more content grows that in proportion. Each reload (every 10 seconds when the game has committed a
change) briefly builds a second copy of the data before swapping, so budget about twice the
figure (an estimate from how reload works, not a measurement). The binary is about 9 MB.

## Download and verify

Pick the newest version from the repository's Releases page; `VERSION` is the tag without the `v`.

```sh
VERSION=0.2.0
REPO=jnfisher/touchstone
cd /tmp
curl -fsSLO "https://github.com/$REPO/releases/download/v$VERSION/touchstone-$VERSION-aarch64-unknown-linux-musl.tar.gz"
curl -fsSLO "https://github.com/$REPO/releases/download/v$VERSION/SHA256SUMS"
sha256sum -c --ignore-missing SHA256SUMS
```

`--ignore-missing` is there because `SHA256SUMS` also lists the x86_64 archive, which you did not
download. The check must print `OK` for the archive you have.

## Install

```sh
tar -xzf touchstone-$VERSION-aarch64-unknown-linux-musl.tar.gz
cd touchstone-$VERSION-aarch64-unknown-linux-musl
./touchstone --version
sudo install -m 0755 touchstone /usr/local/bin/touchstone
sudo install -m 0644 touchstone.service /etc/systemd/system/touchstone.service
sudo install -d -m 0755 /etc/touchstone
sudo install -m 0644 touchstone.example.toml /etc/touchstone/touchstone.toml
```

## The user and the database permissions

Create a dedicated user with no login and no home, and add it to the group that owns the game's
files (check with `ls -l` on the database; `quicksilver service install --system` runs the game as
the `quicksilver` user and group):

```sh
sudo useradd --system --no-create-home --shell /usr/sbin/nologin touchstone
sudo usermod -aG quicksilver touchstone
```

What touchstone needs from the game's files is exactly this, because it opens the database
read-only (`mode=ro`) and the game keeps it in WAL mode:

- **Read permission on the database file, and on its `-wal` and `-shm` files.** These two exist
  only while the game has the database open. Read-only readers use them to see recent commits.
  If the game creates them with mode 0600 (owner only), touchstone fails with "unable to open
  database file"; the game's umask must give the group (or everyone) read access, for example
  a umask of 0027 or 0022 in the game's unit. They need not be writable by touchstone: a reader
  with read-only access to all three files works, even from a read-only mount.
- **Search (x) permission on every directory on the path**, so the group must reach it. Raspberry
  Pi OS home directories are often mode 0700; if the game lives under one, `chmod g+x` it or
  move the game to `/srv`.
- **The game must be running first.** If the game has stopped cleanly, the `-wal` and `-shm`
  files are removed, and a reader that cannot write the directory cannot recreate them; it fails
  with "attempt to write a readonly database". The unit orders itself after `quicksilver.service`
  (rename it in the unit if yours differs) and restarts on failure, so it comes up once the game
  does. If you want touchstone to start without the game running, give the group write permission
  on the database directory and change `ReadOnlyPaths=-/srv/mud` in the unit to
  `ReadWritePaths=/srv/mud`. File permissions still stop it from writing the database.

touchstone never writes to the game's files. Its own files (saved characters, converted
databases) go in `/var/lib/touchstone`, which systemd creates for it (`StateDirectory`).

## Configure

Edit `/etc/touchstone/touchstone.toml`. The two lines to set:

```toml
data_dir = "/var/lib/touchstone"

[source]
kind = "silvermere"
path = "/opt/quicksilver/data/quicksilver.db"   # the game's SQLite file
```

Put the directory that holds the database in `ReadOnlyPaths=` in
`/etc/systemd/system/touchstone.service` (it is `/srv/mud` as shipped); with
`ProtectSystem=strict` the service can read it but nothing outside `/var/lib/touchstone` is
writable. Then:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now touchstone
systemctl status touchstone
journalctl -u touchstone -e
curl -s localhost:8370/api/info
```

Open `http://<pi-address>:8370/` in a browser.

### Using a stock MME database instead

Use `kind = "mdb"`. A converted `.sqlite` needs nothing else. A `.mdb` is converted on start into
`/var/lib/touchstone/converted/` and needs `sudo apt install mdbtools python3` and the
`tools/mdb2sqlite.py` script (from the source repository) beside the binary, or its path in
`converter =`. Only do this if you must load a `.mdb`; otherwise skip `mdbtools`.

## Network

touchstone has no authentication. Keep port 8370 on your LAN or on Tailscale and never forward it
from the router. To restrict it, bind to one address in the config (`bind = "100.x.y.z:8370"`
for the Tailscale address), or with `ufw`:

```sh
sudo ufw allow from 192.168.1.0/24 to any port 8370 proto tcp
```

## Alternative: a user service next to a per-user game (silver-mirror)

If the game runs as a `systemd --user` unit under your own account (Quicksilver in
`~/quicksilver`, as on silver-mirror), the system install above does not fit: home directories are
usually mode 0700, so a separate `touchstone` user cannot reach the database. Run touchstone as
your own user instead, the same way as the game. No `sudo` is needed except to enable lingering.

silver-mirror runs Mealie on Tailscale Serve at `https://<host>.<tailnet>.ts.net/`, wayfarer on
port 7373 and Quicksilver on 2323 and 8080. touchstone takes **7374**, bound to the Tailscale
address only. Check a port is free first with `ss -ltnp | grep 7374`.

Install the binary and write the config (after downloading and verifying the archive as above):

```sh
B=~/.local/share/touchstone
mkdir -p "$B/data"
install -m 0755 touchstone-$VERSION-aarch64-unknown-linux-musl/touchstone "$B/touchstone"
TS_IP=$(tailscale ip -4)
cat > "$B/touchstone.toml" <<EOF
# Tailscale only: touchstone has no authentication.
bind = "$TS_IP:7374"
data_dir = "$B/data"

[source]
kind = "silvermere"
path = "$HOME/quicksilver/data/quicksilver.db"
EOF
```

Write `~/.config/systemd/user/touchstone.service`:

```ini
[Unit]
Description=touchstone (MMUD Explorer web server)
# The game must be running first so its -wal/-shm files exist for the read-only reader.
After=network-online.target quicksilver.service
Wants=network-online.target
StartLimitIntervalSec=0

[Service]
WorkingDirectory=%h/.local/share/touchstone
ExecStart=%h/.local/share/touchstone/touchstone serve --config %h/.local/share/touchstone/touchstone.toml
Restart=on-failure
RestartSec=5
UMask=0027

[Install]
WantedBy=default.target
```

`After=quicksilver.service` only orders units within the same manager, so it works here because
the game is also a user unit. `Restart=on-failure` with no start limit covers the case where the
game's database is not ready yet at boot.

Enable it, and make sure lingering is on so user units start at boot without a login:

```sh
sudo loginctl enable-linger "$USER"     # `loginctl show-user $USER -p Linger` shows Linger=yes if already on
systemctl --user daemon-reload
systemctl --user enable --now touchstone
journalctl --user -u touchstone -e
curl -s "http://$(tailscale ip -4):7374/api/info"
```

Open `http://<host>:7374/` from any machine on the tailnet. To update, replace
`~/.local/share/touchstone/touchstone` and run `systemctl --user restart touchstone`. To uninstall,
`systemctl --user disable --now touchstone`, then remove the unit file and
`~/.local/share/touchstone` (saved characters are in its `data/`).

## Update

```sh
# download and verify the new version as above, then:
tar -xzf touchstone-$VERSION-aarch64-unknown-linux-musl.tar.gz
sudo install -m 0755 touchstone-$VERSION-aarch64-unknown-linux-musl/touchstone /usr/local/bin/touchstone
sudo systemctl restart touchstone
```

Compare `touchstone-$VERSION-*/touchstone.example.toml` with your config for new keys, and
`touchstone.service` with the installed unit; copy the unit again (and `daemon-reload`) if it
changed, keeping your `ReadOnlyPaths=` edit.

## Uninstall

```sh
sudo systemctl disable --now touchstone
sudo rm /etc/systemd/system/touchstone.service /usr/local/bin/touchstone
sudo systemctl daemon-reload
sudo rm -r /etc/touchstone /var/lib/touchstone   # saved characters live in the latter
sudo userdel touchstone
```

## For maintainers: cutting a release

Push a tag `vX.Y.Z` matching the version in `Cargo.toml`. The `release` workflow builds the web UI
once, builds both musl targets with `cargo-zigbuild` and `--features embed-web --release --locked`,
and publishes `touchstone-<version>-<target>.tar.gz` files and `SHA256SUMS`. Running it by hand
(`workflow_dispatch`) is a dry run: it attaches the archives to the run and publishes nothing.
