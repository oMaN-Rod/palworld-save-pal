# palstudio

The [PalStudio](https://github.com/oMaN-Rod/palworld-save-pal) server — a
Palworld save-editor backend with the web UI served built-in.

```
cargo install --git https://github.com/oMaN-Rod/palworld-save-pal palstudio
palstudio
# → http://127.0.0.1:5174
```

On first run the binary downloads the matching GitHub release's UI and
game-data bundle into the platform data home
(`~/.local/share/palstudio` on Linux, `~/Library/Application Support/palstudio`
on macOS, `%APPDATA%\palstudio` on Windows), verifies its SHA-256 against the
release checksums, and starts. Later runs reuse the provisioned assets.

## Options

```
--host <IP>      bind address (default 127.0.0.1; 0.0.0.0 exposes the LAN)
--port <PORT>    listen port (default 5174)
--data-home DIR  root for ui/, data/ and the database
--ui-dir DIR     explicit built-UI dir (skips provisioning)
--data-dir DIR   explicit game-data dir (skips provisioning)
--db FILE        SQLite database file (default <data-home>/ps-rs.db)
--dev            debug logging
```

`PALSTUDIO_REPO=<owner>/<name>` overrides the GitHub repository assets are
provisioned from (defaults to `oMaN-Rod/palworld-save-pal`).

Building from source requires a C/C++ compiler (the Oodle decompressor and
Lua 5.4 are compiled in-tree). GPL-3.0-only.
