from palworld_save_pal.editor.preset_profile import PalPreset, PresetProfile


class TestPalPreset:
    def test_create_minimal(self):
        pp = PalPreset(lock=True)
        assert pp.lock is True
        assert pp.lock_element is False
        assert pp.level is None
        assert pp.character_id is None

    def test_create_full(self):
        pp = PalPreset(
            lock=False,
            lock_element=True,
            character_id="Lambball",
            level=50,
            talent_hp=100,
            passive_skills=["Legend", "Musclehead"],
        )
        assert pp.character_id == "Lambball"
        assert pp.level == 50
        assert pp.passive_skills == ["Legend", "Musclehead"]


class TestPresetProfile:
    def test_create_minimal(self):
        pp = PresetProfile(name="TestPreset", type="pal")
        assert pp.name == "TestPreset"
        assert pp.type == "pal"
        assert pp.skills is None
        assert pp.pal_preset is None

    def test_create_with_pal_preset(self):
        pal_preset = PalPreset(lock=True, level=25)
        pp = PresetProfile(
            name="WithPal", type="pal", pal_preset=pal_preset
        )
        assert pp.pal_preset is not None
        assert pp.pal_preset.level == 25

    def test_model_dump(self):
        pp = PresetProfile(name="Test", type="player", skills=["skill1"])
        d = pp.model_dump()
        assert d["name"] == "Test"
        assert d["type"] == "player"
        assert d["skills"] == ["skill1"]
