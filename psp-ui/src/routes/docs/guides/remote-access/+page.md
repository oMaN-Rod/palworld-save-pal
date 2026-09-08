---
title: Remote Access
description: Pair a phone or tablet to view your world live from another device
---

# Remote Access

Palworld Save Pal can stream a live view of your world to a phone or tablet — pair a device once, then watch the map update in real time while you're away from your PC.

## What Signal Is {.toc}

**Signal** is the pairing and live-view system built into Palworld Save Pal. It has three parts:

- **Pairing** — scan a QR code (or type a short code) on your phone to link it to a running desktop.
- **Live map** — once paired, your phone opens a live map of the loaded world that follows the game in real time.
- **Remote mode** — the desktop can stream from a local save file or from a managed server it's running, so a paired phone can watch either.

Open **Signal** from the navigation bar on the desktop app to start pairing and manage connected devices.

---

## Privacy Posture {.toc}

- **Pairing is authenticated.** A pairing code (or QR code) is required to link a new device, and it expires quickly if unused.
- **The connection is end-to-end encrypted.** Once paired, your phone and PC talk directly over a WebRTC connection secured with DTLS — the same encryption family used by TLS.
- **The pairing server never sees your world data.** A small broker only helps your phone and PC find each other; it relays a sealed handshake between them and cannot read it.
- **The relay forwards ciphertext only.** When a network is too restrictive for a direct connection, traffic is automatically relayed through a TURN server instead. That relay only ever carries already-encrypted traffic — it cannot decrypt or inspect what it forwards.
- **You stay in control.** Nothing can connect while remote access is off, you can revoke a single paired device at any time, and "Reset remote access" cuts every device off and requires each one to pair again.

---

## Self-Hosting Notes {.toc}

### Allowing Remote Pairing on a Self-Host

By default, pairing and remote-access controls can only be started from the same machine the app is running on (loopback only) — a safeguard against turning brief LAN reach into durable remote access. If you're self-hosting Palworld Save Pal on a box you administer from another machine (a home server, a container host, etc.), set:

```
PSP_SIGNAL_ALLOW_REMOTE_PAIRING=1
```

on the machine running Palworld Save Pal to allow pairing and remote-access controls from a non-loopback connection. Only set this on a host you trust to reach that machine's admin surface.

### Docker Builds and Local Saves

Docker (and other web-mode) builds never expose local-save browsing over the network — listing or browsing save directories on the host filesystem is gated to desktop mode only. A self-hosted Docker instance can still stream from a save you upload or from a managed server it runs, but it will not let a remote device browse the host's local save folders.
