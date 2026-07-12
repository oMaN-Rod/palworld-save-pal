"""Generate steam-id conversion vectors from the Python reference implementation."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from palworld_save_pal.game.steam_id import player_uid_to_nosteam, steam_id_to_player_uid

STEAM_IDS = [76561197960265728, 76561198000000001, 76561199999999999, 1, 4242424242]

vectors = []
for steam_id in STEAM_IDS:
    palworld_uid = steam_id_to_player_uid(steam_id)
    vectors.append(
        {
            "steam_id": steam_id,
            "palworld_uid": str(palworld_uid),
            "nosteam_uid": player_uid_to_nosteam(palworld_uid),
        }
    )

out_path = os.path.join("psp-core", "tests", "fixtures", "steam_id_vectors.json")
os.makedirs(os.path.dirname(out_path), exist_ok=True)
with open(out_path, "w") as f:
    json.dump(vectors, f, indent=2)
print(f"wrote {out_path}")
