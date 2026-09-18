use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Account {
    pub user_id: u64,
    pub name: String,
    pub is_premium: bool,
    pub is_supporter: bool,
    pub profile_url: Option<String>,
}

#[derive(Deserialize)]
struct AccountFields {
    user_id: u64,
    name: String,
    #[serde(default)]
    is_premium: Option<bool>,
    #[serde(default, rename = "is_premium?")]
    is_premium_legacy: Option<bool>,
    #[serde(default)]
    is_supporter: bool,
    #[serde(default)]
    profile_url: Option<String>,
}

pub fn parse_account(value: Value) -> Result<Account, serde_json::Error> {
    let fields: AccountFields = serde_json::from_value(value)?;
    Ok(Account {
        user_id: fields.user_id,
        name: fields.name,
        is_premium: fields
            .is_premium
            .or(fields.is_premium_legacy)
            .unwrap_or(false),
        is_supporter: fields.is_supporter,
        profile_url: fields.profile_url,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Category {
    pub category_id: u32,
    pub name: String,
    pub parent_category: Option<u32>,
}

#[derive(Deserialize)]
struct CategoryFields {
    category_id: u32,
    name: String,
    #[serde(default)]
    parent_category: Value,
}

pub fn categories_from_game(mut game: Value) -> Result<Vec<Category>, serde_json::Error> {
    let list = game
        .get_mut("categories")
        .map(Value::take)
        .unwrap_or(Value::Array(Vec::new()));
    let fields: Vec<CategoryFields> = serde_json::from_value(list)?;
    Ok(fields
        .into_iter()
        .map(|field| Category {
            category_id: field.category_id,
            name: field.name,
            parent_category: field
                .parent_category
                .as_u64()
                .and_then(|parent| u32::try_from(parent).ok()),
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DownloadLink {
    pub name: String,
    pub short_name: String,
    #[serde(rename = "URI")]
    pub uri: String,
}

pub fn error_message(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()?
        .get("message")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_account_reads_either_premium_flag_and_drops_the_key() {
        let both = parse_account(json!({
            "user_id": 7, "key": "SECRET", "name": "Tester", "is_premium?": false,
            "is_premium": true, "is_supporter": true, "email": "t@example.invalid",
            "profile_url": "https://example.invalid/u/7"
        }))
        .unwrap();
        assert_eq!(both.user_id, 7);
        assert!(both.is_premium && both.is_supporter);
        assert!(!serde_json::to_string(&both).unwrap().contains("SECRET"));
        let legacy =
            parse_account(json!({ "user_id": 8, "name": "Old", "is_premium?": true })).unwrap();
        assert!(legacy.is_premium);
        assert!(!legacy.is_supporter);
    }

    #[test]
    fn categories_treat_false_as_no_parent() {
        let categories = categories_from_game(json!({ "categories": [
            { "category_id": 1, "name": "Palworld", "parent_category": false },
            { "category_id": 10, "name": "Pals", "parent_category": 1 },
            { "category_id": 11, "name": "Odd" }
        ] }))
        .unwrap();
        assert_eq!(categories[0].parent_category, None);
        assert_eq!(categories[1].parent_category, Some(1));
        assert_eq!(categories[2].parent_category, None);
        assert!(categories_from_game(json!({})).unwrap().is_empty());
    }

    #[test]
    fn download_links_and_error_messages_parse() {
        let links: Vec<DownloadLink> = serde_json::from_value(json!([
            { "name": "Nexus CDN", "short_name": "Nexus CDN", "URI": "https://cf-files.example.invalid/a.zip?md5=x" }
        ]))
        .unwrap();
        assert_eq!(links[0].uri, "https://cf-files.example.invalid/a.zip?md5=x");
        assert_eq!(
            error_message(br#"{"message":"Please provide a valid API Key"}"#).as_deref(),
            Some("Please provide a valid API Key")
        );
        assert_eq!(error_message(b"<html>"), None);
    }
}
