use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::files::ModFile;

pub const GAME_ID: &str = "6063";
pub const MAX_PAGE: u32 = 50;
pub const MAX_OFFSET: u32 = 10_000;
pub const MAX_UPDATE_BATCH: usize = 50;

const SEARCH_QUERY: &str = "query PalStudioSearch($filter: ModsFilter, $sort: [ModsSort!], $offset: Int, $count: Int) { mods(filter: $filter, sort: $sort, offset: $offset, count: $count) { totalCount nodes { modId name summary version author uploader { name } pictureUrl thumbnailUrl endorsements downloads fileSize adultContent createdAt updatedAt category } } }";
const MOD_FILE_FIELDS: &str =
    "fileId name version category date sizeInBytes uri primary description";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
    Relevance,
    Downloads,
    Endorsements,
    Updated,
    Created,
    Name,
}

fn default_count() -> u32 {
    20
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sort: Option<SearchSort>,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_count")]
    pub count: u32,
    #[serde(default)]
    pub include_adult: bool,
}

fn non_blank(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

pub fn search_body(params: &SearchParams) -> Value {
    let query = non_blank(params.query.as_deref());
    let mut filter = Map::new();
    filter.insert(
        "gameId".to_string(),
        json!([{ "value": GAME_ID, "op": "EQUALS" }]),
    );
    if let Some(query) = query {
        filter.insert(
            "name".to_string(),
            json!([{ "value": query, "op": "WILDCARD" }]),
        );
    }
    if let Some(category) = non_blank(params.category.as_deref()) {
        filter.insert(
            "categoryName".to_string(),
            json!([{ "value": category, "op": "EQUALS" }]),
        );
    }
    if !params.include_adult {
        filter.insert(
            "adultContent".to_string(),
            json!([{ "value": false, "op": "EQUALS" }]),
        );
    }
    let sort = params.sort.unwrap_or(if query.is_some() {
        SearchSort::Relevance
    } else {
        SearchSort::Downloads
    });
    let (field, direction) = match sort {
        SearchSort::Relevance => ("relevance", "DESC"),
        SearchSort::Downloads => ("downloads", "DESC"),
        SearchSort::Endorsements => ("endorsements", "DESC"),
        SearchSort::Updated => ("updatedAt", "DESC"),
        SearchSort::Created => ("createdAt", "DESC"),
        SearchSort::Name => ("name", "ASC"),
    };
    let mut order = Map::new();
    order.insert(field.to_string(), json!({ "direction": direction }));
    json!({
        "query": SEARCH_QUERY,
        "variables": {
            "filter": filter,
            "sort": [order],
            "offset": params.offset.min(MAX_OFFSET),
            "count": params.count.clamp(1, MAX_PAGE),
        }
    })
}

pub fn mod_files_body(mod_ids: &[u32]) -> Value {
    let ids: BTreeSet<u32> = mod_ids.iter().copied().collect();
    let mut query = String::from("query PalStudioModFiles {");
    for id in ids {
        let _ = write!(
            query,
            " m{id}: modFiles(modId: \"{id}\", gameId: \"{GAME_ID}\") {{ {MOD_FILE_FIELDS} }}"
        );
    }
    query.push_str(" }");
    json!({ "query": query })
}

pub fn mod_files_from_data(
    data: Value,
    mod_ids: &[u32],
) -> Result<BTreeMap<u32, Vec<ModFile>>, serde_json::Error> {
    let mut object = match data {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    let mut files = BTreeMap::new();
    for id in mod_ids {
        match object.remove(&format!("m{id}")) {
            None | Some(Value::Null) => {}
            Some(Value::Array(entries)) => {
                let parsed = entries
                    .into_iter()
                    .filter_map(|entry| serde_json::from_value::<ModFile>(entry).ok())
                    .collect();
                files.insert(*id, parsed);
            }
            Some(other) => {
                files.insert(*id, serde_json::from_value(other)?);
            }
        }
    }
    Ok(files)
}

#[derive(Debug, Deserialize)]
pub struct GraphqlResponse<T> {
    pub data: Option<T>,
    #[serde(default)]
    pub errors: Vec<GraphqlError>,
}

#[derive(Debug, Deserialize)]
pub struct GraphqlError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchData {
    pub mods: ModsPage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ModsPage {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub nodes: Vec<ModSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uploader {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ModSummary {
    pub mod_id: u32,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub uploader: Option<Uploader>,
    #[serde(default)]
    pub picture_url: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub endorsements: u64,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub file_size: Option<u64>,
    #[serde(default)]
    pub adult_content: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params() -> SearchParams {
        SearchParams {
            query: None,
            category: None,
            sort: None,
            offset: 0,
            count: 20,
            include_adult: false,
        }
    }

    #[test]
    fn a_text_search_filters_by_name_and_hides_adult_mods() {
        let body = search_body(&SearchParams {
            query: Some("  bag ".to_string()),
            count: 500,
            offset: 20_000,
            ..params()
        });
        let variables = &body["variables"];
        assert!(body["query"]
            .as_str()
            .unwrap()
            .contains("mods(filter: $filter"));
        assert_eq!(
            variables["filter"]["gameId"],
            json!([{ "value": "6063", "op": "EQUALS" }])
        );
        assert_eq!(
            variables["filter"]["name"],
            json!([{ "value": "bag", "op": "WILDCARD" }])
        );
        assert_eq!(
            variables["filter"]["adultContent"],
            json!([{ "value": false, "op": "EQUALS" }])
        );
        assert!(variables["filter"].get("categoryName").is_none());
        assert_eq!(
            variables["sort"],
            json!([{ "relevance": { "direction": "DESC" } }])
        );
        assert_eq!(variables["count"], 50);
        assert_eq!(variables["offset"], 10_000);
    }

    #[test]
    fn browsing_sorts_by_downloads_and_can_include_adult_mods() {
        let body = search_body(&SearchParams {
            category: Some("Pals".to_string()),
            include_adult: true,
            count: 0,
            ..params()
        });
        let variables = &body["variables"];
        assert_eq!(
            variables["filter"]["categoryName"],
            json!([{ "value": "Pals", "op": "EQUALS" }])
        );
        assert!(variables["filter"].get("adultContent").is_none());
        assert!(variables["filter"].get("name").is_none());
        assert_eq!(
            variables["sort"],
            json!([{ "downloads": { "direction": "DESC" } }])
        );
        assert_eq!(variables["count"], 1);
        let by_name = search_body(&SearchParams {
            sort: Some(SearchSort::Name),
            ..params()
        });
        assert_eq!(
            by_name["variables"]["sort"],
            json!([{ "name": { "direction": "ASC" } }])
        );
        let updated = search_body(&SearchParams {
            sort: Some(SearchSort::Updated),
            ..params()
        });
        assert_eq!(
            updated["variables"]["sort"],
            json!([{ "updatedAt": { "direction": "DESC" } }])
        );
    }

    #[test]
    fn search_params_deserialize_from_a_message_payload() {
        let parsed: SearchParams =
            serde_json::from_value(json!({ "query": "x", "sort": "endorsements", "offset": 40 }))
                .unwrap();
        assert_eq!(parsed.sort, Some(SearchSort::Endorsements));
        assert_eq!(parsed.offset, 40);
        assert_eq!(parsed.count, 20);
        assert!(!parsed.include_adult);
    }

    #[test]
    fn a_search_page_deserializes() {
        let data = json!({ "mods": { "totalCount": 3, "nodesCount": 3, "nodes": [{
            "modId": 4222, "name": "Sekhmet", "summary": "New Pals", "version": "1.2",
            "author": "TBoxBR", "uploader": { "name": "TBoxBR" },
            "pictureUrl": "https://example.invalid/p.png", "thumbnailUrl": null,
            "endorsements": 804, "downloads": 84107, "fileSize": 605184, "adultContent": true,
            "updatedAt": "2026-08-17T18:18:29Z", "createdAt": "2026-07-23T09:03:24Z", "category": "Pals"
        }] } });
        let page: SearchData = serde_json::from_value(data).unwrap();
        assert_eq!(page.mods.total_count, 3);
        let node = &page.mods.nodes[0];
        assert_eq!(
            (node.mod_id, node.downloads, node.adult_content),
            (4222, 84107, true)
        );
        assert_eq!(node.file_size, Some(605_184));
        assert_eq!(
            node.uploader.as_ref().map(|u| u.name.as_str()),
            Some("TBoxBR")
        );
        let out = serde_json::to_value(node).unwrap();
        assert_eq!(out["mod_id"], 4222);
        assert_eq!(out["picture_url"], "https://example.invalid/p.png");
        assert_eq!(out["file_size"], 605_184);
    }

    #[test]
    fn mod_files_are_batched_by_alias_and_read_back() {
        let body = mod_files_body(&[4821, 12, 4821]);
        let query = body["query"].as_str().unwrap();
        assert!(
            query.contains(r#"m12: modFiles(modId: "12", gameId: "6063")"#),
            "{query}"
        );
        assert_eq!(query.matches("m4821: modFiles(").count(), 1);
        assert!(query.contains("sizeInBytes"));
        let data = json!({
            "m12": [{ "fileId": 7, "name": "a", "version": "1", "category": "MAIN",
                      "sizeInBytes": "10", "uri": "a.zip", "primary": 1 }],
            "m4821": null
        });
        let files = mod_files_from_data(data, &[12, 4821, 99]).unwrap();
        assert_eq!(files.keys().copied().collect::<Vec<_>>(), vec![12]);
        assert_eq!(files[&12][0].file_id, 7);
    }

    #[test]
    fn an_unparseable_file_entry_does_not_drop_the_others() {
        let data = json!({
            "m12": [
                { "fileId": 7, "name": "a", "version": "1", "category": "MAIN",
                  "sizeInBytes": "10", "uri": "a.zip", "primary": 1 },
                { "name": "no file id", "category": "MAIN", "uri": "b.zip" },
                { "fileId": 8, "name": "c", "version": "2", "category": "MAIN",
                  "sizeInBytes": "20", "uri": "c.zip", "primary": 0 },
            ]
        });
        let files = mod_files_from_data(data, &[12]).unwrap();
        let ids: Vec<u32> = files[&12].iter().map(|file| file.file_id).collect();
        assert_eq!(ids, vec![7, 8]);
    }
}
