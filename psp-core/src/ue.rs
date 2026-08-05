//! `uesave` types specialized to Palworld.
//!
//! `uesave` is now generic over a [`uesave::Game`]; its `Save`, `Property`,
//! `StructValue`, ... all default to `NoGame`, whose game-struct variant is
//! uninhabited. Palworld's typed `RawData` structs only exist when the game
//! type is [`Palworld`], so the whole crate must thread it. This module aliases
//! the game-parameterized types to `Palworld` and re-exports everything else
//! from `uesave`, so the rest of the crate can refer to `crate::ue::*` uniformly.

pub use uesave::*;

pub use uesave::games::palworld::{compression, CompressionFormat, PalStruct, Palworld};

/// The Palworld archive type parameter carried by every value type below.
pub type Arch = uesave::SaveGameArchiveType<Palworld>;

pub type Save = uesave::Save<Palworld>;
pub type Property = uesave::Property<Arch>;
pub type Properties = uesave::Properties<Arch>;
pub type StructValue = uesave::StructValue<Arch>;
pub type ValueVec = uesave::ValueVec<Arch>;
pub type MapEntry = uesave::MapEntry<Arch>;
pub type Root = uesave::Root<Arch>;

/// Resolve a struct-type name the way the reader does. A Palworld game struct
/// (`PalCharacterContainer`, `PalDynamicItem`, ...) resolves to
/// [`uesave::StructType::Game`], which the write hook recognizes and serializes
/// back into its embedded byte array; every other name falls back to
/// [`uesave::StructType::from`]. Priming a game struct as a plain named struct
/// instead makes uesave write it with the wrong codec — a save that no longer
/// parses back.
pub fn struct_type_for(name: &str) -> uesave::StructType {
    if <Palworld as uesave::Game>::is_game_struct_type(name) {
        uesave::StructType::Game(name.to_owned())
    } else {
        uesave::StructType::from(name)
    }
}
