from enum import Enum, IntEnum
from typing import Optional, Tuple
from palworld_save_pal.utils.logging_config import create_logger

logger = create_logger(__name__)


class PrefixedEnum(Enum):
    @classmethod
    def _prefix(cls):
        return getattr(cls, "_enum_prefix", f"{cls.__name__}::")

    def prefixed(self):
        return f"{self._enum_prefix.value}{self.value}"


class ArrayType(str, Enum):
    BYTE_PROPERTY = "ByteProperty"
    ENUM_PROPERTY = "EnumProperty"
    NAME_PROPERTY = "NameProperty"
    STRUCT_PROPERTY = "StructProperty"


class EntryState(str, Enum):
    NONE = "None"
    MODIFIED = "Modified"
    NEW = "New"
    DELETED = "Deleted"


class Element(str, Enum):
    """Element types in the game"""

    NEUTRAL = "Normal"
    DARK = "Dark"
    DRAGON = "Dragon"
    ICE = "Ice"
    FIRE = "Fire"
    GRASS = "Leaf"
    GROUND = "Earth"
    ELECTRIC = "Electricity"
    WATER = "Water"
    UNKNOWN = "Unknown"

    @classmethod
    def get_all_elements(cls) -> list[str]:
        """Get all element type values"""
        return [member.value for member in cls]

    @classmethod
    def from_value(cls, value: str) -> "Element":
        """Convert from game's enum format to our enum"""
        type_str = value.split("::")[-1]
        try:
            return next((t for t in cls if t.value == type_str), cls.UNKNOWN)
        except KeyError:
            return cls.UNKNOWN


class GroupType(str, PrefixedEnum):
    _enum_prefix = "EPalGroupType::"

    GUILD = "Guild"
    ORGANIZATION = "Organization"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(GroupType._enum_prefix.value, "")
            return GroupType(value)
        except Exception:
            logger.warning("%s is not a valid group type", value)


class PalGender(str, PrefixedEnum):
    _enum_prefix = "EPalGenderType::"

    NONE = "None"
    MALE = "Male"
    FEMALE = "Female"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(PalGender._enum_prefix.value, "")
            return PalGender(value)
        except Exception:
            logger.warning("%s is not a valid gender, defaulting to female", value)
            return PalGender.FEMALE


class PalRank(int, Enum):
    RANK0 = 1
    RANK1 = 2
    RANK2 = 3
    RANK3 = 4
    RANK4 = 5

    def get_index(self):
        return self.value - 1

    @staticmethod
    def from_value(value: int):
        try:
            return PalRank(value)
        except Exception:
            logger.warning("%s is not a valid rank", value)


class WorkSuitability(str, PrefixedEnum):
    _enum_prefix = "EPalWorkSuitability::"

    EMIT_FLAME = "EmitFlame"
    WATERING = "Watering"
    SEEDING = "Seeding"
    GENERATE_ELECTRICITY = "GenerateElectricity"
    HANDCRAFT = "Handcraft"
    COLLECTION = "Collection"
    DEFOREST = "Deforest"
    MINING = "Mining"
    OIL_EXTRACTION = "OilExtraction"
    PRODUCT_MEDICINE = "ProductMedicine"
    COOL = "Cool"
    TRANSPORT = "Transport"
    MONSTER_FARM = "MonsterFarm"

    @staticmethod
    def from_value(value: str):
        try:
            value = value.replace(WorkSuitability._enum_prefix.value, "")
            return WorkSuitability(value)
        except Exception:
            logger.warning("%s is not a valid work suitability", value)


class WazaID(IntEnum):
    """Skill ID enum mapping numeric values to names"""

    NONE = 0
    Human_Punch = 1
    WorkAttack = 2
    Throw = 3
    Scratch = 4
    EnergyShot = 5
    Unique_Anubis_LowRoundKick = 6
    Unique_Anubis_GroundPunch = 7
    Unique_Anubis_Tackle = 8
    Unique_Deer_PushupHorn = 9
    HyperBeam = 10
    PowerShot = 11
    PowerBall = 12
    Unique_Garm_Bite = 13
    Intimidate = 14
    Unique_Boar_Tackle = 15
    Unique_PinkCat_CatPunch = 16
    Unique_FlowerDinosaur_Whip = 17
    Unique_SheepBall_Roll = 18
    Unique_ChickenPal_ChickenPeck = 19
    Unique_Gorilla_GroundPunch = 20
    Unique_Grassmammoth_Earthquake = 21
    AirCanon = 22
    Unique_GrassPanda_MusclePunch = 23
    Unique_RobinHood_BowSnipe = 24
    Unique_Alpaca_Tackle = 25
    Unique_KingAlpaca_BodyPress = 26
    Unique_Werewolf_Scratch = 27
    Unique_FengyunDeeper_CloudTempest = 28
    Unique_Baphomet_SwallowKite = 29
    Unique_HerculesBeetle_BeetleTackle = 30
    Unique_HawkBird_Storm = 31
    Unique_Eagle_GlidingNail = 32
    SelfDestruct = 33
    SelfDestruct_Bee = 34
    SelfExplosion = 35
    Unique_Garm_BiteV2 = 36
    Unique_GuardianDog_Bite = 37
    Unique_GuardianDog_BiteV2 = 38
    RadiantBarrage = 39
    FireBlast = 40
    Flamethrower = 41
    FireBall = 42
    FlareArrow = 43
    FireSeed = 44
    Unique_Horus_FlareBird = 45
    FlareTornado = 46
    Inferno = 47
    Unique_FireKirin_Tackle = 48
    Unique_AmaterasuWolf_FireCharge = 49
    Unique_VolcanicMonster_MagmaAttack = 50
    Unique_FlameBuffalo_FlameHorn = 51
    Eruption = 52
    FlameWall = 53
    FlameFunnel = 54
    Unique_AmaterasuWolf_Bite = 55
    Unique_AmaterasuWolf_BiteV2 = 56
    WaterGun = 57
    WaterWave = 58
    HydroPump = 59
    WaterBall = 60
    TidalWave = 61
    AquaJet = 62
    BubbleShot = 63
    AcidRain = 64
    SeaGush = 65
    RipTide = 66
    DiversionLaser = 67
    HydroSlicer = 68
    Unique_BluePlatypus_Toboggan = 69
    Unique_TentacleTurtle_HydroSpin = 70
    Unique_SakuraSaurus_Water_SplashTackle = 71
    WindCutter = 72
    GrassTornado = 73
    SolarBeam = 74
    SeedMachinegun = 75
    SeedMine = 76
    RootAttack = 77
    SpecialCutter = 78
    CrossWind = 79
    ReflectiveShuriken = 80
    HealingTree = 81
    Unique_QueenBee_SpinLance = 82
    ThunderRain = 83
    ThunderBall = 84
    LineThunder = 85
    CrossThunder = 86
    ThreeThunder = 87
    ElecWave = 88
    Thunderbolt = 89
    ThunderFunnel = 90
    SpreadPulse = 91
    LockonLaser = 92
    LightningStrike = 93
    ThunderSpear = 94
    Unique_ElecPanda_ElecScratch = 95
    Unique_Kirin_LightningTackle = 96
    Unique_FlowerDinosaur_Electric_ThunderWhip = 97
    Unique_ThunderDog_Bite = 98
    Unique_ThunderDog_BiteV2 = 99
    IceMissile = 100
    BlizzardLance = 101
    SnowStorm = 102
    IcicleThrow = 103
    IceBlade = 104
    Unique_IceHorse_IceBladeAttack = 105
    Unique_IceNarwhal_JumpingHorn = 106
    Unique_KingAlpaca_Ice_IcePress = 107
    SandTornado = 108
    ThrowRock = 109
    RockLance = 110
    MudShot = 111
    StoneShotgun = 112
    Unique_DrillGame_ShellAttack = 113
    Unique_Deer_Ground_DirtyHorn = 114
    Unique_Gorilla_Ground_EarthPunch = 115
    Unique_GoldenHorse_Bite = 116
    Unique_GoldenHorse_BiteV2 = 117
    DarkLaser = 118
    DarkWave = 119
    ShadowBall = 120
    Psychokinesis = 121
    PoisonShot = 122
    GhostFlame = 123
    GravityShot = 124
    Unique_DarkCrow_TelePoke = 125
    Unique_Baphomet_Dark_DarkKite = 126
    Unique_IceHorse_Dark_DarkBladeAttack = 127
    Unique_AmaterasuWolf_Dark_Bite = 128
    Unique_AmaterasuWolf_Dark_BiteV2 = 129
    Unique_BlackPuppy_Bite = 130
    Unique_BlackPuppy_BiteV2 = 131
    DragonMeteor = 132
    DragonBreath = 133
    DragonWave = 134
    DragonCanon = 135
    Unique_FairyDragon_FairyTornado = 136
    Funnel_DreamDemon = 137
    Funnel_RaijinDaughter = 138
    StardustArrow = 139
    Tremor = 140
    FrostBreath = 141
    DiamondFall = 142
    BeamSlicer = 143
    Commet = 144
    DarkBall = 145
    PoisonFog = 146
    DarkLegion = 147
    DarkCanon = 148
    DarkArrow = 149
    DarkPulse = 150
    Apocalypse = 151
    StarMine = 152
    AirBlade = 153
    HolyBlast = 154
    RootLance = 155
    LineGeyser = 156
    WallSplash = 157
    TriSpark = 158
    ThunderStorm = 159
    SandTwister = 160
    IcicleLine = 161
    ThreeCommet = 162
    CommetRain = 163
    BlastCanon = 164
    ChargeCanon = 165
    RangeThunder = 166
    Railbolt = 167
    ShokeiLaser = 168
    BubbleShower = 169
    WaterBalloon = 170
    IciclePierce = 171
    DoubleIcicleThrow = 172
    IceAge = 173
    RaidCutter = 174
    WindEdge = 175
    FlareTwister = 176
    TrisRing = 177
    Unique_BirdDragon_FireBreath = 178
    Unique_BlackMetalDragon_FirePunch = 179
    Unique_DarkScorpion_Pierce = 180
    Unique_GhostBeast_Tossin = 181
    Unique_JetDragon_JumpBeam = 182
    Unique_ThunderBird_ThunderStorm = 183
    Unique_Yeti_SnowBall = 184
    Unique_NaughtyCat_CatPress = 185
    Unique_IceDeer_IceHorn = 186
    Unique_KingBahamut_AirCrash = 187
    Unique_Manticore_InfernoStrike = 188
    Unique_SoldierBee_NeedleLance = 189
    Unique_ThunderDog_InazumaShorai = 190
    Unique_BlackCentaur_TwoSpearRushes = 191
    Unique_BlackGriffon_TackleLaser = 192
    Unique_SakuraSaurus_SideTackle = 193
    Unique_ThunderDragonMan_ThunderSwordAttack = 194
    Unique_RedArmorBird_TriplePeck = 195
    Unique_CaptainPenguin_BodySlide = 196
    Unique_CaptainPenguin_Black_BodySlide_Electric = 197
    Unique_Ronin_Iai = 198
    Unique_GrassRabbitMan_GrassRoundKick = 199
    Unique_SaintCentaur_OneSpearRushes = 200
    Unique_Umihebi_WindingTackle = 201
    Unique_WeaselDragon_FlyingTackle = 202
    Unique_WhiteTiger_IceScratch = 203
    Unique_IceCrocodile_SpitAttack = 204
    Unique_BirdDragon_Ice_IceBreath = 205
    Unique_FireKirin_Dark_DarkTossin = 206
    Unique_VolcanicMonster_Ice_IceAttack = 207
    Unique_LeafMomonga_SomerSault = 208
    Unique_Yeti_Grass_GrassBall = 209
    Unique_GrassPanda_Electric_ElectricPunch = 210
    Unique_NightLady_WarpBeam = 211
    Unique_NightLady_WarpBeam_Straight = 212
    Unique_NightLady_FlameNightmare = 213
    Unique_MoonQueen_MoonBeam = 214
    Unique_MoonQueen_MoonBlade = 215
    Unique_KingBahamut_ArmSmash = 216
    Unique_WingGolem_RoundCutter = 217
    Unique_ScorpionMan_Uppercut = 218
    Unique_FeatherOstrich_Tossin = 219
    Unique_DarkAlien_JumpScractch = 220
    Unique_SifuDog_Counter = 221
    Unique_ThunderDragonMan_NumerousSwordAttack = 222
    Unique_ElecPanda_GatlingAttack = 223
    Unique_LilyQueen_LilyHealing = 224
    Unique_LilyQueen_WindBarrier = 225
    Unique_Horus_PerfectStorm = 226
    Unique_BlackGriffon_TackleLaser2 = 227
    Unique_MoonQueen_IceMoonBlade = 228
    Unique_DarkMechaDragon_SetFunnel = 229
    Unique_DarkMechaDragon_ConvergentBeam = 230
    Unique_DarkMechaDragon_FunnelLaser = 231
    Unique_DarkMechaDragon_BeamSlash = 232
    Unique_DarkMechaDragon_WarpComet = 233
    Unique_Umihebi_Fire_FireWindingTackle = 234
    Unique_PurpleSpider_SpiderRaid = 235
    Unique_MysteryMask_LifeSteal = 236
    Unique_GrimGirl_BrutalMachete = 237
    Unique_SnowTigerBeastman_TrampleSlash = 238
    Unique_SnowTigerBeastman_SnowImpact = 239
    Unique_WhiteShieldDragon_ShieldTackle = 240
    Unique_NightBlueHorse_DeathStep = 241
    Unique_BlueThunderHorse_FlashDash = 242
    Unique_WhiteDeer_HolyPillar = 243
    Unique_GoldenHorse_StoneDash = 244
    Unique_WhiteTiger_Ground_IronScratch = 245
    Unique_FengyunDeeper_Electric_ThunderTempest = 246
    Unique_Werewolf_Ice_SnowScratch = 247
    Unique_Horus_Water_AquaStorm = 248
    Unique_AmaterasuWolf_Dark_DarkCharge = 249
    Unique_OctopursGirl_InkJet = 250
    Unique_StuffedShark_HiddenWeapon = 251
    Unique_Plesiosaur_LongBreath = 252
    Unique_TropicalOstrich_DashKick = 253
    Unique_GhostAnglerfish_SweepBait = 254
    Unique_GhostAnglerfish_Fire_SweepBait_Fire = 255
    Unique_PoseidonOrca_TorrentLaser = 256
    Unique_VolcanoDragon_VolcanicLaser = 257
    Unique_VolcanoDragon_MagmaSpit = 258
    Unique_Sekhmet_RollingScratch = 259
    Unique_Sekhmet_SomersaultScratch = 260
    Unique_LegendDeer_WarpPillarBurst = 261
    Unique_LegendDeer_BarrierRelease_Normal = 262
    Unique_LegendDeer_BarrierRelease_Grass = 263
    Unique_LegendDeer_BarrierRelease_Water = 264
    Unique_LegendDeer_RadiantPurge = 265
    Unique_LegendDeer_RadiantWingRush = 266
    Unique_LegendDeer_RadiantPurge_Otomo = 267
    Unique_Yakushima_SummonServant = 268
    Unique_Yakushima_EyeTossin = 269
    Unique_Yakushima_MouthTossin = 270
    Unique_YakushimaMonster001_SlimePress_Normal = 271
    Unique_YakushimaMonster001_SlimePress_Leaf = 272
    Unique_YakushimaMonster001_SlimePress_Water = 273
    Unique_YakushimaMonster001_SlimePress_Fire = 274
    Unique_YakushimaMonster001_SlimePress_Dark = 275
    Unique_YakushimaMonster001_SlimePress_Rainbow = 276
    Unique_YakushimaBoss001_Small_DemonEyeCharge = 277
    Unique_YakushimaMonster002_SwordCharge = 278
    Unique_YakushimaMonster003_BatCharge = 279
    PredatorBeam = 280
    PredatorWave = 281
    PredatorLockon = 282
    RockBeat = 283
    IceWall = 284
    Funnel_RaijinDaughter_Water = 285
    BlueThunderHorse_PartnerSkill = 286
    Unique_YakushimaBoss001_Green_PhantasmalBolt = 287
    Unique_YakushimaBoss001_Green_PhantasmalEye = 288
    Unique_YakushimaBoss001_Green_PhantasmalSphere = 289
    Unique_YakushimaBoss001_Green_PhantasmalDeathray = 290
    Unique_YakushimaBoss002_PhantasmalBolt = 291
    Unique_YakushimaBoss002_PhantasmalEye = 292
    Unique_YakushimaBoss002_PhantasmalSphere = 293
    Unique_YakushimaBoss002_PhantasmalDeathray = 294
    PoseidonOrca_PartnerSkill_SpearBullet = 295
    PoseidonOrca_PartnerSkill = 296
    Unique_BluePlatypus_Toboggan_Fire = 297
    Human_Rolling = 298
    Weapon_Use = 299
    Unique_YakushimaBoss001_Green_2_PhantasmalBolt = 300
    Unique_YakushimaBoss001_Green_2_PhantasmalEye = 301
    Unique_YakushimaBoss001_Green_2_PhantasmalSphere = 302
    Unique_YakushimaBoss001_Green_2_PhantasmalDeathray = 303
    Unique_YakushimaBoss002_2_PhantasmalBolt = 304
    Unique_YakushimaBoss002_2_PhantasmalEye = 305
    Unique_YakushimaBoss002_2_PhantasmalSphere = 306
    Unique_YakushimaBoss002_2_PhantasmalDeathray = 307
    Unique_NightBlueHorse_Tossin = 308
    Unique_BlueThunderHorse_Tossin = 309
    MAX = 310

    @classmethod
    def from_str(cls, value: str) -> Tuple[Optional["WazaID"], str]:
        """
        Convert from game's enum format (e.g., 'EPalWazaID::266' or 'EPalWazaID::Unique_LegendDeer_RadiantWingRush').

        Returns a tuple of (WazaID or None, skill_id_string).
        The skill_id_string is always prefixed with 'EPalWazaID::' for consistency.
        """
        if not value.startswith("EPalWazaID::"):
            return None, value

        id_part = value.split("::")[-1]

        # Try to parse as a name first
        try:
            waza = cls[id_part]
            return waza, f"EPalWazaID::{waza.name}"
        except KeyError:
            pass

        # Try to parse as a numeric value
        try:
            numeric_id = int(id_part)
            waza = cls(numeric_id)
            return waza, f"EPalWazaID::{waza.name}"
        except (ValueError, KeyError):
            pass

        # Both lookups failed
        return None, value

    def to_str(self) -> str:
        """Convert back to game's enum format with name"""
        return f"EPalWazaID::{self.name}"
