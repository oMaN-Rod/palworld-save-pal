# Third-Party Notices

PalStudio's own source is licensed under the [GNU General Public License v3.0](LICENSE).
That grant covers the code in this repository. It does not cover the third-party
material described below, which is included so the tool can read, render and explain
Palworld save files.

## Palworld game data and derived assets

PalStudio is a fan-made save editor. To display a save it needs to know what the
game's Pals, items, skills, buildings and world look like, so this repository
carries data and 3D assets derived from Palworld:

| Path | Size | Tracked files | Contents |
| --- | --- | --- | --- |
| `ps-ui/static/models` | ~86 MB | 1,738 | Converted meshes for Pals, structures, scenery and map objects |
| `ps-ui/static/maps` | ~43 MB | 1,366 | World heightmap, map tiles and scenery placement data |
| `data/json` | ~27 MB | 272 | Game data tables and localisation strings |

**Palworld, its game data, artwork, models, names and world are the property of
Pocketpair, Inc.** They are not the authors' work, are not licensed under the GPL,
and are not placed under it by their inclusion here. No ownership is claimed over
them, and nothing in the GPL grant above should be read as sublicensing them.

This material is included solely to make an interoperable tool for people who own
the game. It is not a substitute for Palworld and cannot be used to play it.

PalStudio is not affiliated with, endorsed by, or sponsored by Pocketpair, Inc.
Palworld is a trademark of Pocketpair, Inc.

If you represent a rights holder and want material removed, open an issue on the
project's GitHub repository and it will be addressed.

## Bundled software

PS Amity, the optional in-game bridge, vendors its own dependencies. Their licences
are listed separately in [`ps-amity/THIRD_PARTY_NOTICES.md`](ps-amity/THIRD_PARTY_NOTICES.md).

PalStudio downloads, but does not redistribute, the following at the user's request:

- [UE4SS](https://github.com/Okaetsu/RE-UE4SS) — mod loader, fetched from its GitHub releases
- [PalSchema](https://github.com/Okaetsu/PalSchema) — mod framework, fetched from its GitHub releases
- Mods obtained through [Nexus Mods](https://www.nexusmods.com/palworld), under Nexus Mods' terms

Rust and JavaScript dependencies are declared in `Cargo.toml` and the various
`package.json` files, and retain their own licences.
