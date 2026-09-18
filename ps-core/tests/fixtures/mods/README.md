# Manifest Routing Fixtures

Golden fixtures for the manifest builder routing engine.

## Fixture Format

```json
{
  "archive_name": "Cool 100 2 2026-01-01T00-00Z abc.zip",
  "platform": "win64",
  "modinfo": null,
  "entries": ["(Steam)/Cool_P.pak", "(Xbox)/Cool_P.pak"],
  "expect": {
    "mod_type": "pak",
    "folder_name": "Cool_P",
    "platform_filtered": "win64",
    "routes": [["(Steam)/Cool_P.pak", "pak", "Cool_P.pak"]],
    "decisions": ["pak_destination"]
  }
}
```

`routes` are `[archive_path, kind, rel_path]` triples and must match the manifest's routes exactly, order-insensitive. `decisions` lists the `kind` tags of expected decisions in order.

Add one fixture per routing rule; name them `NN_rule.json`.
