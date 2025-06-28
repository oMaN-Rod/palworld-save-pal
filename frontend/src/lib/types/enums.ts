// This file provides TypeScript enums for all domain enums used in the frontend.
// These should match the values in your backend and app logic.

export enum ItemTypeA {
  None = 'None',
  Weapon = 'Weapon',
  SpecialWeapon = 'SpecialWeapon',
  Armor = 'Armor',
  Accessory = 'Accessory',
  Material = 'Material',
  Consume = 'Consume',
  Ammo = 'Ammo',
  Food = 'Food',
  Essential = 'Essential',
  Glider = 'Glider',
  MonsterEquipWeapon = 'MonsterEquipWeapon',
  Blueprint = 'Blueprint'
}

export enum ItemTypeB {
  None = 'None',
  WeaponMelee = 'WeaponMelee',
  WeaponBow = 'WeaponBow',
  WeaponCrossbow = 'WeaponCrossbow',
  WeaponHandgun = 'WeaponHandgun',
  WeaponAssaultRifle = 'WeaponAssaultRifle',
  WeaponSniperRifle = 'WeaponSniperRifle',
  WeaponRocketLauncher = 'WeaponRocketLauncher',
  WeaponShotgun = 'WeaponShotgun',
  WeaponFlameThrower = 'WeaponFlameThrower',
  WeaponGatlingGun = 'WeaponGatlingGun',
  WeaponCollectionTool = 'WeaponCollectionTool',
  WeaponThrowObject = 'WeaponThrowObject',
  WeaponGrapplingGun = 'WeaponGrapplingGun',
  SPWeaponCaptureBall = 'SPWeaponCaptureBall',
  SPWeaponDamageTrap = 'SPWeaponDamageTrap',
  SPWeaponCaptureTrap = 'SPWeaponCaptureTrap',
  SPWeaponCaptureRope = 'SPWeaponCaptureRope',
  ArmorHead = 'ArmorHead',
  ArmorBody = 'ArmorBody',
  Accessory = 'Accessory',
  MaterialOre = 'MaterialOre',
  MaterialJewelry = 'MaterialJewelry',
  MaterialIngot = 'MaterialIngot',
  MaterialWood = 'MaterialWood',
  MaterialStone = 'MaterialStone',
  MaterialProccessing = 'MaterialProccessing',
  MaterialMonster = 'MaterialMonster',
  MaterialPalEgg = 'MaterialPalEgg',
  ConsumeBandage = 'ConsumeBandage',
  ConsumeSeed = 'ConsumeSeed',
  ConsumeBullet = 'ConsumeBullet',
  ConsumeWazaMachine = 'ConsumeWazaMachine',
  ConsumeTechnologyBook = 'ConsumeTechnologyBook',
  ConsumeAncientTechnologyBook = 'ConsumeAncientTechnologyBook',
  ConsumeOther = 'ConsumeOther',
  ConsumeGainStatusPoints = 'ConsumeGainStatusPoints',
  ConsumePalLevelUp = 'ConsumePalLevelUp',
  ConsumePalGainExp = 'ConsumePalGainExp',
  ConsumePalTalentUp = 'ConsumePalTalentUp',
  ConsumePalRankUp = 'ConsumePalRankUp',
  FoodMeat = 'FoodMeat',
  FoodVegetable = 'FoodVegetable',
  FoodFish = 'FoodFish',
  FoodDishMeat = 'FoodDishMeat',
  FoodDishVegetable = 'FoodDishVegetable',
  FoodDishFish = 'FoodDishFish',
  FoodProcessed = 'FoodProcessed',
  Essential = 'Essential',
  Essential_UnlockPlayerFuture = 'Essential_UnlockPlayerFuture',
  Glider = 'Glider',
  Shield = 'Shield',
  Money = 'Money',
  Medicine = 'Medicine',
  Drug = 'Drug',
  MonsterEquipWeapon = 'MonsterEquipWeapon',
  Blueprint = 'Blueprint',
  ReturnToBaseCamp = 'ReturnToBaseCamp',
  Essential_PalGear = 'Essential_PalGear'
}

export enum Rarity {
  Common = 'Common',
  Uncommon = 'Uncommon',
  Rare = 'Rare',
  Epic = 'Epic',
  Legendary = 'Legendary'
}

export enum BuildingTypeA {
  Product = 'Product',
  Pal = 'Pal',
  Storage = 'Storage',
  Food = 'Food',
  Infrastructure = 'Infrastructure',
  Light = 'Light',
  Foundation = 'Foundation',
  Defense = 'Defense',
  Other = 'Other',
  Furniture = 'Furniture',
  Dismantle = 'Dismantle',
  EPalBuildObjectTypeA_MAX = 'EPalBuildObjectTypeA_MAX'
}

export enum BuildingTypeB {
  Prod_Craft = 'Prod_Craft',
  Prod_Resource = 'Prod_Resource',
  Prod_Furnace = 'Prod_Furnace',
  Prod_Medicine = 'Prod_Medicine',
  Pal_Capture = 'Pal_Capture',
  Pal_Breed = 'Pal_Breed',
  Pal_Modify = 'Pal_Modify',
  Infra_Medical = 'Infra_Medical',
  Infra_Storage = 'Infra_Storage',
  Infra_Trade = 'Infra_Trade',
  Infra_GeneratePower = 'Infra_GeneratePower',
  Infra_Defense = 'Infra_Defense',
  Infra_Environment = 'Infra_Environment',
  Food_Basic = 'Food_Basic',
  Food_Agriculture = 'Food_Agriculture',
  Food_Cooking = 'Food_Cooking',
  Food_Livestock = 'Food_Livestock',
  Found_Basic = 'Found_Basic',
  Found_House = 'Found_House',
  Other = 'Other',
  EPalBuildObjectTypeB_MAX = 'EPalBuildObjectTypeB_MAX'
}

export enum PalGender {
  MALE = 'Male',
  FEMALE = 'Female'
}

export enum CharacterContainerType {
  PAL_BOX = 'PalBox',
  PARTY = 'Party',
  BASE = 'Base'
}

export enum SaveFileType {
  GAMEPASS = 'gamepass',
  STEAM = 'steam'
}

export enum ElementType {
  Fire = 'Fire',
  Water = 'Water',
  Ground = 'Ground',
  Ice = 'Ice',
  Neutral = 'Neutral',
  Dark = 'Dark',
  Grass = 'Grass',
  Dragon = 'Dragon',
  Electric = 'Electric'
}
