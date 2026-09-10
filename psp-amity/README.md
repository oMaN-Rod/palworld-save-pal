# PSP Amity

In-game bridge mod for Palworld (UE4SS C++ mod). Hosts a local, token-authenticated
WebSocket that PSP connects to for live game state.

The everyday path is the repo-root launcher: `.\dev.ps1 -Amity` builds the mod and
installs it into the Palworld install Steam reports, with the game closed. Its `-Check`
form lists what is missing. The scripts underneath take every machine-specific value as
an argument and default to an `amity-build` workspace folder beside the repo:

- `scripts/setup-workspace.ps1 [-Root <dir>]` — one-time dev setup: clones the pinned UE4SS
  fork into a CMake workspace and junctions this directory into it as `PSPAmity`. The mod's
  CMakeLists.txt links the `UE4SS` target, so it only configures as a subdirectory of that
  workspace.
- `scripts/build.ps1 [-Root <dir>] [-Targets PSPAmity,AmityCoreTests]` — build the mod DLL,
  and optionally the core test runner. cmake is taken from PATH or the Visual Studio install.
- `scripts/install-local.ps1 -GameDir <...\steamapps\common\Palworld> [-Root <dir>]
  [-Ue4ssZip <zip>]` — build, then copy the mod into whichever UE4SS instance the game has
  (Workshop or Win64); the zip is only needed to create a Win64 instance from scratch.
- `scripts/package-mod.ps1 [-Root <dir>]` — produce the distributable mod zip, versioned
  from `mod/amity_mod.hpp`
- `tools/probe.ts` — bun client for driving the bridge by hand: every read and write op has
  a subcommand, and `reflect`, `find` and `holder` answer reflection questions over the live
  socket. It discovers the running instance from `%LOCALAPPDATA%\Pal\Saved\PSPAmity\endpoints\`
  and performs the v2 nonce/HMAC handshake; pass `--pid=<pid>` to pick a specific instance when
  more than one is running. Run it with no arguments for a usage list.

The mod's startup resolution report (`[PSPAmity] resolve ...` lines in UE4SS.log) checks
every game symbol the ops depend on against the running build and ends with a one-line
`ok`/`MISSING` count. `docs/` holds the build identifiers and the live-verification evidence
for each milestone, including the reflection findings the guild ops rest on.
