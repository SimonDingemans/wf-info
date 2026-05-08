use std::{collections::HashMap, fs, path::Path};

use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::rewards::RewardOverlayEntry;

pub type Result<T> = std::result::Result<T, ItemDatabaseError>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemDatabase {
    items: Vec<Item>,
}

impl ItemDatabase {
    pub fn from_files(
        prices_path: impl AsRef<Path>,
        filtered_items_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let prices_path = prices_path.as_ref();
        let filtered_items_path = filtered_items_path.as_ref();
        let prices =
            fs::read_to_string(prices_path).map_err(|source| ItemDatabaseError::ReadFile {
                path: prices_path.to_path_buf(),
                source,
            })?;
        let filtered_items = fs::read_to_string(filtered_items_path).map_err(|source| {
            ItemDatabaseError::ReadFile {
                path: filtered_items_path.to_path_buf(),
                source,
            }
        })?;

        Self::from_json(&prices, &filtered_items)
    }

    pub fn from_cache_dir(cache_dir: impl AsRef<Path>) -> Result<Self> {
        let cache_dir = cache_dir.as_ref();

        Self::from_files(
            cache_dir.join("prices.json"),
            cache_dir.join("filtered_items.json"),
        )
    }

    pub fn from_json(prices: &str, filtered_items: &str) -> Result<Self> {
        let price_table = load_prices(prices)?;
        let filtered_items = load_filtered_items(filtered_items)?;
        let mut items = process_items(
            filtered_items.eqmt,
            filtered_items.ignored_items,
            &price_table,
        );

        apply_special_price_overrides(&mut items);

        Ok(Self { items })
    }

    pub fn new(items: Vec<Item>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn find_item(&self, text: &str, threshold: Option<usize>) -> Option<&Item> {
        let needle = searchable_item_name(text);
        let needle_is_set = needle.ends_with("set");

        self.items
            .iter()
            .filter(|item| needle_is_set || !item.name.ends_with(" Set"))
            .filter_map(|item| {
                let distance =
                    levenshtein_distance(&searchable_item_name(&item.drop_name), &needle);
                let current_threshold = threshold.unwrap_or(item.drop_name.len() / 3);

                (distance <= current_threshold).then_some((item, distance))
            })
            .min_by_key(|(_, distance)| *distance)
            .map(|(item, _)| item)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub drop_name: String,
    pub platinum: f32,
    pub ducats: u32,
    pub volume: u32,
    pub vaulted: bool,
}

impl Item {
    pub fn set_platinum(&mut self, platinum: f32) {
        self.platinum = platinum;
    }

    pub fn reward_overlay_entry(&self) -> RewardOverlayEntry {
        RewardOverlayEntry::name_only(self.drop_name.clone())
            .with_platinum(self.platinum.round().max(0.0) as u32)
            .with_ducats(self.ducats)
            .with_vaulted(self.vaulted)
    }
}

#[derive(Debug, Error)]
pub enum ItemDatabaseError {
    #[error("could not read item database file {path}: {source}")]
    ReadFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("could not parse prices payload: {0}")]
    PricesJson(serde_json::Error),
    #[error("could not parse filtered items payload: {0}")]
    FilteredItemsJson(serde_json::Error),
}

#[derive(Clone, Debug, Deserialize)]
struct PriceItem {
    name: String,
    #[serde(deserialize_with = "deserialize_f32_from_string_or_number")]
    custom_avg: f32,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u32_from_string_or_number"
    )]
    yesterday_vol: Option<u32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_u32_from_string_or_number"
    )]
    today_vol: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct DucatItem {
    #[serde(default)]
    ducats: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum EquipmentType {
    Warframes,
    Primary,
    Secondary,
    Melee,
    Sentinels,
    Archwing,
    #[serde(rename = "Arch-Gun")]
    ArchGun,
    Skins,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
struct EquipmentItem {
    #[serde(rename = "type")]
    item_type: EquipmentType,
    #[serde(default)]
    vaulted: bool,
    #[serde(default)]
    parts: HashMap<String, DucatItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct FilteredItems {
    #[serde(default)]
    eqmt: HashMap<String, EquipmentItem>,
    #[serde(default)]
    ignored_items: HashMap<String, DucatItem>,
}

fn load_prices(prices: &str) -> Result<HashMap<String, PriceItem>> {
    let prices: Vec<PriceItem> =
        serde_json::from_str(prices).map_err(ItemDatabaseError::PricesJson)?;

    Ok(prices
        .into_iter()
        .map(|price| (price.name.clone(), price))
        .collect())
}

fn load_filtered_items(filtered_items: &str) -> Result<FilteredItems> {
    serde_json::from_str(filtered_items).map_err(ItemDatabaseError::FilteredItemsJson)
}

fn process_items(
    equipment: HashMap<String, EquipmentItem>,
    ignored_items: HashMap<String, DucatItem>,
    price_table: &HashMap<String, PriceItem>,
) -> Vec<Item> {
    equipment
        .into_iter()
        .flat_map(|(equipment_name, equipment_item)| {
            let set_item = set_item(&equipment_name, &equipment_item, price_table);

            equipment_item
                .parts
                .into_iter()
                .filter_map(move |(name, ducat_item)| {
                    equipment_part_item(
                        name,
                        ducat_item,
                        equipment_item.item_type,
                        equipment_item.vaulted,
                        price_table,
                    )
                })
                .chain(set_item)
        })
        .chain(ignored_items.into_iter().map(|(name, ducat_item)| {
            let price = price_table.get(&name);

            Item {
                name: name.clone(),
                drop_name: name,
                platinum: price.map(|price| price.custom_avg).unwrap_or_default(),
                ducats: ducat_item.ducats,
                volume: price.map(total_recent_volume).unwrap_or_default(),
                vaulted: false,
            }
        }))
        .collect()
}

fn set_item(
    equipment_name: &str,
    equipment_item: &EquipmentItem,
    price_table: &HashMap<String, PriceItem>,
) -> Option<Item> {
    let set_name = format!("{equipment_name} Set");
    let price = price_table.get(&set_name);

    if price.is_none() {
        log::warn!("failed to find price for item: {set_name}");
    }

    price.map(|price| Item {
        name: set_name.clone(),
        drop_name: set_name,
        platinum: price.custom_avg,
        ducats: 0,
        volume: total_recent_volume(price),
        vaulted: equipment_item.vaulted,
    })
}

fn equipment_part_item(
    name: String,
    ducat_item: DucatItem,
    equipment_type: EquipmentType,
    vaulted: bool,
    price_table: &HashMap<String, PriceItem>,
) -> Option<Item> {
    let price = price_table
        .get(&name)
        .or_else(|| price_table.get(&format!("{name} Blueprint")));

    if price.is_none() {
        log::warn!("failed to find price for item: {name}");
    }

    let price = price?;

    Some(Item {
        drop_name: drop_name_for_part(&name, equipment_type),
        name,
        platinum: price.custom_avg,
        ducats: ducat_item.ducats,
        volume: total_recent_volume(price),
        vaulted,
    })
}

fn drop_name_for_part(name: &str, equipment_type: EquipmentType) -> String {
    match equipment_type {
        EquipmentType::Warframes | EquipmentType::Archwing
            if is_blueprint_drop_part(name) && !name.ends_with("Blueprint") =>
        {
            format!("{name} Blueprint")
        }
        _ => name.to_owned(),
    }
}

fn is_blueprint_drop_part(name: &str) -> bool {
    name.ends_with("Systems")
        || name.ends_with("Neuroptics")
        || name.ends_with("Chassis")
        || name.ends_with("Harness")
        || name.ends_with("Wings")
}

fn total_recent_volume(price: &PriceItem) -> u32 {
    price.yesterday_vol.unwrap_or_default() + price.today_vol.unwrap_or_default()
}

fn apply_special_price_overrides(items: &mut [Item]) {
    if let Some(item) = items.iter_mut().find(|item| item.name == "Forma Blueprint") {
        item.set_platinum(35.0 / 3.0);
    }
}

fn searchable_item_name(name: &str) -> String {
    name.chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    let mut current = vec![0; previous.len()];

    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;

        for (right_index, right_character) in right.chars().enumerate() {
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            let substitution =
                previous[right_index] + usize::from(left_character != right_character);
            current[right_index + 1] = insertion.min(deletion).min(substitution);
        }

        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.chars().count()]
}

fn deserialize_f32_from_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::Number(number) => number
            .as_f64()
            .map(|number| number as f32)
            .ok_or_else(|| serde::de::Error::custom("expected finite number")),
        serde_json::Value::String(value) => value.parse().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("expected number or string")),
    }
}

fn deserialize_optional_u32_from_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .and_then(|number| number.try_into().ok())
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("expected unsigned integer")),
        Some(serde_json::Value::String(value)) => {
            value.parse().map(Some).map_err(serde::de::Error::custom)
        }
        Some(_) => Err(serde::de::Error::custom("expected number, string, or null")),
    }
}

#[cfg(test)]
mod tests {
    use super::{ItemDatabase, levenshtein_distance};

    #[test]
    fn database_loads_equipment_parts_sets_and_ignored_items() {
        let database =
            ItemDatabase::from_json(prices_json(), filtered_items_json()).expect("database");

        let ash_systems = database
            .find_item("Ash Prime Systems Blueprint", None)
            .expect("warframe systems part");
        assert_eq!(ash_systems.name, "Ash Prime Systems");
        assert_eq!(ash_systems.drop_name, "Ash Prime Systems Blueprint");
        assert_eq!(ash_systems.platinum, 22.0);
        assert_eq!(ash_systems.ducats, 45);
        assert_eq!(ash_systems.volume, 7);
        assert!(ash_systems.vaulted);

        let set = database.find_item("Ash Prime Set", None).expect("set item");
        assert_eq!(set.platinum, 80.0);
        assert_eq!(set.volume, 15);

        let forma = database
            .find_item("Forma Blueprint", None)
            .expect("ignored item");
        assert_eq!(forma.platinum, 35.0 / 3.0);
        assert_eq!(forma.ducats, 0);
        assert!(!forma.vaulted);
    }

    #[test]
    fn database_fuzzy_matches_ocr_text_without_matching_sets_by_default() {
        let database =
            ItemDatabase::from_json(prices_json(), filtered_items_json()).expect("database");

        let item = database
            .find_item("ash prime systerns blueprint", None)
            .expect("fuzzy match");

        assert_eq!(item.drop_name, "Ash Prime Systems Blueprint");

        assert_eq!(database.find_item("Ash Prime", None), None);

        let set = database
            .find_item("Ash Prime Set", None)
            .expect("set should match when set was requested");
        assert_eq!(set.name, "Ash Prime Set");
    }

    #[test]
    fn item_converts_to_reward_overlay_entry() {
        let database =
            ItemDatabase::from_json(prices_json(), filtered_items_json()).expect("database");
        let item = database
            .find_item("Ash Prime Systems Blueprint", None)
            .expect("item");

        let entry = item.reward_overlay_entry();

        assert_eq!(entry.name, "Ash Prime Systems Blueprint");
        assert_eq!(entry.platinum, Some(22));
        assert_eq!(entry.ducats, Some(45));
        assert!(entry.vaulted);
    }

    #[test]
    fn levenshtein_distance_handles_insertions_deletions_and_substitutions() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    fn prices_json() -> &'static str {
        r#"[
            {
                "name": "Ash Prime Set",
                "custom_avg": "80.0",
                "yesterday_vol": "10",
                "today_vol": "5"
            },
            {
                "name": "Ash Prime Systems Blueprint",
                "custom_avg": "22.0",
                "yesterday_vol": "3",
                "today_vol": "4"
            },
            {
                "name": "Ash Prime Neuroptics Blueprint",
                "custom_avg": 18.0,
                "yesterday_vol": 1,
                "today_vol": 2
            },
            {
                "name": "Forma Blueprint",
                "custom_avg": "9.0",
                "yesterday_vol": null,
                "today_vol": "12"
            }
        ]"#
    }

    fn filtered_items_json() -> &'static str {
        r#"{
            "eqmt": {
                "Ash Prime": {
                    "type": "Warframes",
                    "vaulted": true,
                    "parts": {
                        "Ash Prime Systems": { "ducats": 45 },
                        "Ash Prime Neuroptics": { "ducats": 25 }
                    }
                }
            },
            "ignored_items": {
                "Forma Blueprint": { "ducats": 0 }
            }
        }"#
    }
}
