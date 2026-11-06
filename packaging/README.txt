touchstone: MMUD Explorer as a web server. See docs/install-pi.md in the source repository for
setup. Files here:

  touchstone               the server, with the web UI inside it (static binary)
  touchstone.example.toml  every config key, with comments
  touchstone.service       systemd unit (expects /usr/local/bin/touchstone and
                           /etc/touchstone/touchstone.toml)

Run `touchstone --version`, then `touchstone serve --config touchstone.toml`.
There is no authentication: keep the port on a LAN or Tailscale.
