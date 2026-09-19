# Installing PSAmity

PSAmity is a UE4SS C++ mod. It does not run on its own — it loads into an
already-running UE4SS instance inside Palworld. Install UE4SS first, then
install PSAmity into that same UE4SS instance.

> **Already had PSAmity installed?** This release changes the bridge's wire
> protocol and is not compatible with older installs — see section 8,
> "Upgrading from an older PSAmity", before you assume something broke.

## 1. Prerequisite: UE4SS

Palworld ships an official UE4SS build via Steam Workshop. Get UE4SS onto
your game installation one of two ways:

- **Steam Workshop (recommended):** subscribe to the game's official UE4SS
  workshop mod. Steam syncs it into
  `<GameDir>\Mods\NativeMods\UE4SS\`.
- **Manual install:** unzip the UE4SS release bundle directly next to
  `Palworld-Win64-Shipping.exe` (`<GameDir>\Pal\Binaries\Win64\`), so that
  `dwmapi.dll` and the `ue4ss\` folder sit alongside the game executable.

Only use one of these at a time — see the warning below.

## 2. CRITICAL: only one UE4SS instance

Check first whether `<GameDir>\Mods\NativeMods\UE4SS\` already exists (the
Steam Workshop route). If it does, **do not** also install UE4SS manually
into `Pal\Binaries\Win64\`. Running two UE4SS instances at once crashes the
game at startup.

- If the Workshop UE4SS is present, install PSAmity into **that** instance's
  `Mods` folder. Do not add a second `dwmapi.dll` / `ue4ss\` under Win64.
- Only use the manual Win64 install if the Workshop UE4SS is not present.

## 3. Unzip PSAmity

Download `PSAmity-UE4SS-<version>.zip` and unzip it so that its `PSAmity`
folder lands inside the UE4SS `Mods` directory of whichever instance you are
using:

- Workshop route:
  `<GameDir>\Mods\NativeMods\UE4SS\Mods\PSAmity\`
- Manual/Win64 route:
  `<GameDir>\Pal\Binaries\Win64\ue4ss\Mods\PSAmity\`

After unzipping you should have `Mods\PSAmity\dlls\main.dll`,
`Mods\PSAmity\enabled.txt` and `Mods\PSAmity\THIRD_PARTY_NOTICES.md` under one of
those `Mods` directories.

The game's in-game Mod Management screen only lists Steam Workshop
packages. A sideloaded UE4SS mod like PSAmity will not appear there — that
is expected and does not mean the install failed.

## 4. Verify

Launch Palworld, load into a world, then check `UE4SS.log` for lines
showing PSAmity loaded and its bridge listening. `UE4SS.log` is written
inside the folder holding `UE4SS.dll`:

- Workshop/native route: `<GameDir>\Mods\NativeMods\UE4SS\UE4SS.log`
- Manual/Win64 route: `<GameDir>\Pal\Binaries\Win64\ue4ss\UE4SS.log`

Example lines:

```
[PSAmity] loaded v0.3.1
[PSAmity] unreal initialized
[PSAmity] bridge listening on 127.0.0.1:<port> as "PSAmity"
```

The last line now names both the bind address and the instance name (`"PSAmity"` by
default, or whatever `name` is set to in `PSAmity.ini` — see section 6).

Once a world is loaded the log also carries a `[PSAmity] resolve ...` line per game
symbol the mod depends on, ending in `resolution report complete: <n> ok, <m> MISSING`.
A MISSING entry means the game build has changed under the mod; the affected operation
reports itself unavailable rather than guessing.

If these lines are missing, confirm PSAmity's files are under the correct
`Mods` folder for whichever UE4SS instance is actually active (Workshop vs.
manual), and that UE4SS itself loaded successfully earlier in the log.

## 5. Discovery (how PalStudio finds the running game)

Once loaded, PSAmity writes a small connection file to
`%LOCALAPPDATA%\Pal\Saved\PSAmity\endpoints\<pid>.json`, named after its own
process id. Each running instance gets its own file in that `endpoints`
folder, so a solo game and a dedicated server running on the same machine at
the same time are both discovered correctly instead of one overwriting the
other's connection info. With no configuration at all, a single local
instance is found automatically and nothing needs setting up.

Each file records the protocol version, the port, the token, the instance
name, the bind address, the process id and the launch time. PalStudio checks the
recorded process id against the running processes, so a file left behind by
a crashed instance is ignored rather than treated as live — no manual
cleanup is needed.

## 6. Configuration (optional)

PSAmity reads settings from `PSAmity.ini`, which lives in the mod's own
folder — `Mods\PSAmity\PSAmity.ini` under whichever `Mods` directory you
installed into (see section 3), alongside `dlls\`, `enabled.txt` and
`THIRD_PARTY_NOTICES.md`. The zip ships this file with every setting
commented out. Left untouched, the mod behaves exactly as it always has: a
loopback-only bridge on an OS-assigned port with a fresh random token every
launch, discovered automatically by PalStudio with no setup.

All settings live under a `[bridge]` section:

| Key     | Default                              | Meaning |
|---------|---------------------------------------|---------|
| `name`  | `PSAmity`                            | Friendly name shown in PalStudio's instance list. |
| `port`  | `0` (OS picks a free port)            | Fixed port; needed to reach this instance from another machine. |
| `token` | *(blank — a fresh random token every launch)* | Shared secret PalStudio must present to connect. |
| `bind`  | `127.0.0.1`                           | Interface to listen on. `0.0.0.0` accepts connections from the network — see Security below. |

Each setting can also be supplied as an environment variable, which wins over
the file: `PSAMITY_NAME`, `PSAMITY_PORT`, `PSAMITY_TOKEN`, `PSAMITY_BIND`.

Two rules are enforced. Breaking either keeps the bridge from starting at all,
rather than silently doing something else:

- **A non-loopback `bind` requires an explicit `token`.** Without one, the
  bridge refuses to start and says so in `UE4SS.log`.
- **A configured `port` that is already in use makes the bridge fail to
  start, rather than silently choosing a different port.** PSAmity actively
  probes the port before binding to it, because on Windows the socket
  library would otherwise let two servers share the same port silently. So
  the port you set here is always the port you should point PalStudio at.

The shipped example, exactly as it appears in `PSAmity.ini`:

```ini
; PSAmity bridge configuration.
; Every setting below is optional. With this file untouched the mod behaves
; exactly as it did before: a loopback-only bridge on an OS-assigned port with
; a fresh random token each launch, discovered automatically by PalStudio.
;
; Each setting can also be supplied as an environment variable, which wins over
; this file: PSAMITY_NAME, PSAMITY_PORT, PSAMITY_TOKEN, PSAMITY_BIND.

[bridge]

; Friendly name shown in PalStudio's instance list.
; name = Dedicated - world4

; 0 (default) lets the OS pick a free port. A fixed port is required to reach
; this instance from another machine. If the port is already taken the bridge
; does not start rather than silently moving.
; port = 8788

; Shared secret PalStudio must present to connect. Blank (default) means a new random
; token every launch, which only works for automatic local discovery.
; A non-loopback bind REQUIRES an explicit token here.
; token = choose-a-long-random-secret

; 127.0.0.1 (default) accepts connections only from this machine.
; 0.0.0.0 accepts them from the network. Traffic after the login handshake is
; NOT encrypted, so only use this on a LAN or over a VPN.
; bind = 0.0.0.0
```

## 7. Security

Only the login handshake protects the token: PSAmity issues a random nonce,
and PalStudio proves it knows the token by sending back an HMAC of that nonce — the
token itself is never sent over the wire in the clear. But everything that
happens after login — reading pal and player data, and every edit command —
travels over that same WebSocket connection unencrypted. There is no TLS.

Because of that, `bind = 0.0.0.0` belongs on a trusted LAN or over a VPN, and
should never be used on a host exposed directly to the internet. The
handshake keeps the token itself from leaking; it does not make the rest of
the traffic safe on an untrusted network.

## 8. Upgrading from an older PSAmity

This release changes the bridge's wire protocol (v1 to v2), with no backward
compatibility. **The mod and PalStudio must be updated together.** If only one
side is updated, PalStudio reports `protocol_mismatch` for that instance instead of
connecting — that is expected, not a sign that the update broke something.
Reinstall PSAmity from this release (section 3) and make sure PalStudio is
updated to match, and the connection will work as before.

## 9. Co-op clients (non-host players)

If you are a joining client in a co-op session rather than the host, the
bridge is not authoritative for the same data the host sees. Some read
features may be limited or unavailable when connecting as a client rather
than the host; PSAmity fails closed in that case rather than returning
incorrect data.

## Uninstall

Delete the `Mods\PSAmity` folder from whichever UE4SS `Mods` directory you
installed it into.
