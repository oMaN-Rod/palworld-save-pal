# Installing PSPAmity

PSPAmity is a UE4SS C++ mod. It does not run on its own — it loads into an
already-running UE4SS instance inside Palworld. Install UE4SS first, then
install PSPAmity into that same UE4SS instance.

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

- If the Workshop UE4SS is present, install PSPAmity into **that** instance's
  `Mods` folder. Do not add a second `dwmapi.dll` / `ue4ss\` under Win64.
- Only use the manual Win64 install if the Workshop UE4SS is not present.

## 3. Unzip PSPAmity

Download `PSPAmity-UE4SS-<version>.zip` and unzip it so that its `PSPAmity`
folder lands inside the UE4SS `Mods` directory of whichever instance you are
using:

- Workshop route:
  `<GameDir>\Mods\NativeMods\UE4SS\Mods\PSPAmity\`
- Manual/Win64 route:
  `<GameDir>\Pal\Binaries\Win64\ue4ss\Mods\PSPAmity\`

After unzipping you should have `Mods\PSPAmity\dlls\main.dll`,
`Mods\PSPAmity\enabled.txt` and `Mods\PSPAmity\THIRD_PARTY_NOTICES.md` under one of
those `Mods` directories.

The game's in-game Mod Management screen only lists Steam Workshop
packages. A sideloaded UE4SS mod like PSPAmity will not appear there — that
is expected and does not mean the install failed.

## 4. Verify

Launch Palworld, load into a world, then check `UE4SS.log` for lines
showing PSPAmity loaded and its bridge listening. `UE4SS.log` is written
inside the folder holding `UE4SS.dll`:

- Workshop/native route: `<GameDir>\Mods\NativeMods\UE4SS\UE4SS.log`
- Manual/Win64 route: `<GameDir>\Pal\Binaries\Win64\ue4ss\UE4SS.log`

Example lines:

```
[PSPAmity] loaded v0.1.0
[PSPAmity] unreal initialized
[PSPAmity] bridge listening on 127.0.0.1:<port>
```

Once a world is loaded the log also carries a `[PSPAmity] resolve ...` line per game
symbol the mod depends on, ending in `resolution report complete: <n> ok, <m> MISSING`.
A MISSING entry means the game build has changed under the mod; the affected operation
reports itself unavailable rather than guessing.

If these lines are missing, confirm PSPAmity's files are under the correct
`Mods` folder for whichever UE4SS instance is actually active (Workshop vs.
manual), and that UE4SS itself loaded successfully earlier in the log.

## 5. Discovery (how PSP finds the running game)

Once loaded, PSPAmity writes a per-launch connection file to
`%LOCALAPPDATA%\Pal\Saved\PSPAmity\endpoint.json`, containing the protocol
version, the local port, a token generated fresh for that session, the game's
process id and the launch time. PSP reads this file to connect automatically —
no manual configuration is required.

If the game crashes, this file can be left over from the previous session.
PSP checks the process id recorded in the file and will not treat a stale
file as a live connection. The file is rewritten on the next launch, so no
manual cleanup is needed.

## 6. Co-op clients (non-host players)

If you are a joining client in a co-op session rather than the host, the
bridge is not authoritative for the same data the host sees. Some read
features may be limited or unavailable when connecting as a client rather
than the host; PSPAmity fails closed in that case rather than returning
incorrect data.

## Uninstall

Delete the `Mods\PSPAmity` folder from whichever UE4SS `Mods` directory you
installed it into.
