use crate::scrape::Annotation;

/// Fetch annotations from Genius API using Bearer token.
/// Calls `https://api.genius.com/referents?song_id={id}` and parses response.
pub async fn fetch_from_api(
    client: &reqwest::Client,
    token: &str,
    song_id: i64,
) -> Result<Vec<Annotation>, String> {
    let url = format!("https://api.genius.com/referents?song_id={}", song_id);
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API returned {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse failed: {}", e))?;

    let mut annotations = Vec::new();

    if let Some(refs) = data
        .pointer("/response/referents")
        .and_then(|r| r.as_array())
    {
        for referent in refs {
            let fragment = referent
                .get("fragment")
                .and_then(|f| f.as_str())
                .unwrap_or("")
                .to_string();
            let body = referent
                .pointer("/annotations/0/body/dom")
                .and_then(|d| extract_text_from_dom(d))
                .unwrap_or_default();

            if !fragment.is_empty() && !body.is_empty() {
                annotations.push(Annotation { fragment, body });
            }
        }
    }

    if annotations.is_empty() {
        return Err("No annotations found from API".to_string());
    }

    Ok(annotations)
}

/// Extract text from Genius DOM tree structure
fn extract_text_from_dom(dom: &serde_json::Value) -> Option<String> {
    match dom {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(m) => {
            if let Some(children) = m.get("children").and_then(|c| c.as_array()) {
                let mut texts = Vec::new();
                for child in children {
                    if let Some(t) = extract_text_from_dom(child) {
                        texts.push(t);
                    }
                }
                if texts.is_empty() {
                    None
                } else {
                    Some(texts.join("").trim().to_string())
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_from_dom_string() {
        let dom = serde_json::json!("hello");
        assert_eq!(extract_text_from_dom(&dom), Some("hello".to_string()));
    }

    #[test]
    fn test_extract_text_from_dom_object() {
        let dom = serde_json::json!({
            "children": ["hello ", "world"]
        });
        assert_eq!(extract_text_from_dom(&dom), Some("hello world".to_string()));
    }

    #[test]
    fn test_extract_text_from_dom_nested() {
        let dom = serde_json::json!({
            "children": [
                {"children": ["hello"]},
                " ",
                {"children": ["world"]}
            ]
        });
        assert_eq!(extract_text_from_dom(&dom), Some("hello world".to_string()));
    }
}
