//! A minimal insertion-order-preserving map, backing wire DTO fields whose
//! JSON key order comes from the save file's own list order
//! (`PalDto::work_suitability`, `PlayerDto::status_point_list`,
//! `BaseDto::storage_containers`, ...). A `BTreeMap`/`HashMap` field would
//! re-sort or scramble those keys on the wire, and there is no room to carry
//! a separate order list beside the map without changing the JSON shape.
//! Implemented over a `Vec<(K, V)>` with hand-written serde impls so the
//! crate needs no `indexmap` dependency.
use std::borrow::Borrow;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedMap<K, V>(Vec<(K, V)>);

impl<K, V> OrderedMap<K, V> {
    pub fn new() -> Self {
        OrderedMap(Vec::new())
    }

    fn with_capacity(capacity: usize) -> Self {
        OrderedMap(Vec::with_capacity(capacity))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
}

impl<K: PartialEq, V> OrderedMap<K, V> {
    /// Re-inserting an existing key updates its value in place; the key keeps
    /// its original position.
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(existing) = self.0.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = value;
        } else {
            self.0.push((key, value));
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.0
            .iter()
            .find(|(k, _)| k.borrow() == key)
            .map(|(_, v)| v)
    }
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        OrderedMap(Vec::new())
    }
}

impl<K, V> FromIterator<(K, V)> for OrderedMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        OrderedMap(iter.into_iter().collect())
    }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Serialize, V: Serialize> Serialize for OrderedMap<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, K: Deserialize<'de> + PartialEq, V: Deserialize<'de>> Deserialize<'de>
    for OrderedMap<K, V>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedMapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K: Deserialize<'de> + PartialEq, V: Deserialize<'de>> Visitor<'de>
            for OrderedMapVisitor<K, V>
        {
            type Value = OrderedMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                // Routed through `insert()` rather than pushing onto the
                // backing `Vec` so a repeated JSON key is last-value-wins at
                // the first key's position instead of becoming a second entry.
                let mut map = OrderedMap::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry()? {
                    map.insert(key, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(OrderedMapVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_insertion_order_not_key_order() {
        let mut map: OrderedMap<String, i64> = OrderedMap::new();
        map.insert("Handcraft".to_string(), 1);
        map.insert("EmitFlame".to_string(), 0);
        map.insert("Mining".to_string(), 2);

        let serialized = serde_json::to_string(&map).unwrap();
        assert_eq!(r#"{"Handcraft":1,"EmitFlame":0,"Mining":2}"#, serialized);
    }

    #[test]
    fn re_inserting_an_existing_key_updates_value_without_moving_it() {
        let mut map: OrderedMap<String, i64> = OrderedMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.insert("a".to_string(), 99);

        let serialized = serde_json::to_string(&map).unwrap();
        assert_eq!(r#"{"a":99,"b":2}"#, serialized);
    }

    #[test]
    fn get_accepts_str_key_for_string_map() {
        let mut map: OrderedMap<String, i64> = OrderedMap::new();
        map.insert("Handcraft".to_string(), 1);
        assert_eq!(map.get("Handcraft"), Some(&1));
        assert_eq!(map.get("Missing"), None);
    }

    #[test]
    fn deserialize_preserves_json_source_order() {
        let value = serde_json::json!({"c": 3, "a": 1, "b": 2});
        let map: OrderedMap<String, i64> = serde_json::from_value(value).unwrap();
        let keys: Vec<&String> = map.iter().map(|(k, _)| k).collect();
        assert_eq!(vec!["c", "a", "b"], keys);
    }

    #[test]
    fn deserialize_dedupes_repeated_keys_last_value_wins_at_first_position() {
        // Parsed via `from_str`, not `json!` + `from_value`: the `json!` macro
        // builds a `Value::Object` that already dedupes duplicate keys, so
        // `visit_map` would never see the repeated key at all.
        let map: OrderedMap<String, i64> =
            serde_json::from_str(r#"{"a": 1, "b": 2, "a": 3}"#).unwrap();
        assert_eq!(2, map.len(), "duplicate key must not produce two entries");
        let entries: Vec<(&String, &i64)> = map.iter().collect();
        assert_eq!(
            vec![(&"a".to_string(), &3), (&"b".to_string(), &2)],
            entries
        );
    }

    #[test]
    fn deserialize_then_serialize_round_trip_preserves_order_after_dedup() {
        let map: OrderedMap<String, i64> =
            serde_json::from_str(r#"{"c": 1, "a": 2, "c": 3, "b": 4}"#).unwrap();
        let serialized = serde_json::to_string(&map).unwrap();
        assert_eq!(r#"{"c":3,"a":2,"b":4}"#, serialized);
    }

    #[test]
    fn uuid_keys_serialize_as_strings() {
        let mut map: OrderedMap<uuid::Uuid, i64> = OrderedMap::new();
        let id: uuid::Uuid = "11111111-2222-3333-4444-555555555555".parse().unwrap();
        map.insert(id, 7);
        let serialized = serde_json::to_string(&map).unwrap();
        assert_eq!(r#"{"11111111-2222-3333-4444-555555555555":7}"#, serialized);
    }
}
