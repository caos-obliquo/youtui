use scraper::{Html, Selector};
use serde_json::Value;

/// Fetch a Genius song page and extract lyrics from the HTML.
/// Returns (lyrics, final_url). Validates final URL matches expected slug path.
pub async fn fetch_lyrics(
    client: &reqwest::Client,
    song_path: &str,
) -> Result<(String, String), String> {
    let (html, _, final_url) = fetch_page(client, song_path).await?;
    // Check if the final URL matches the expected song path
    let expected_base = format!("https://genius.com{}", song_path);
    let expected_base_no_lyrics = format!("https://genius.com{}", song_path.trim_end_matches("-lyrics"));
    if !final_url.starts_with(&expected_base) && !final_url.starts_with(&expected_base_no_lyrics) {
        return Err(format!("Redirected to different page: {}", final_url));
    }
    let lyrics = extract_lyrics(&html)?;
    Ok((lyrics, final_url))
}

/// Fetch a Genius song page and extract annotations from embedded JSON state.
pub async fn fetch_annotations(
    client: &reqwest::Client,
    song_path: &str,
) -> Result<Vec<Annotation>, String> {
    let (html, _, _) = fetch_page(client, song_path).await?;
    extract_annotations(&html)
}

/// Check if a Genius page exists at the given path (returns true if status 200).
/// Does NOT validate content — use fetch_lyrics for that.
pub async fn page_exists(client: &reqwest::Client, song_path: &str) -> bool {
    let url = format!("https://genius.com{}", song_path);
    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Fetch page HTML (shared helper).
/// Returns (html, status, final_url). Final URL may differ from requested if redirects occurred.
async fn fetch_page(
    client: &reqwest::Client,
    song_path: &str,
) -> Result<(String, reqwest::StatusCode, String), String> {
    let url = format!("https://genius.com{}", song_path);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Genius page: {}", e))?;
    let status = resp.status();
    let final_url = resp.url().to_string();
    if !status.is_success() {
        return Err(format!("Genius page returned {}", status));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read Genius page body: {}", e))?;
    Ok((html, status, final_url))
}

/// Extract lyrics from Genius HTML page string.
pub fn extract_lyrics(html: &str) -> Result<String, String> {
    let document = Html::parse_document(html);

    let selector = Selector::parse("div[data-lyrics-container=\"true\"]")
        .map_err(|e| format!("Failed to parse selector: {}", e))?;

    let mut sections: Vec<String> = Vec::new();

    for element in document.select(&selector) {
        let text = extract_container_text(&element);
        if !text.trim().is_empty() {
            sections.push(text);
        }
    }

    if sections.is_empty() {
        return Err("No lyrics container found on page".to_string());
    }

    let raw = sections.join("\n");
    let cleaned = clean_lyrics(&raw);

    if cleaned.trim().is_empty() {
        return Err("Lyrics container was empty".to_string());
    }

    Ok(cleaned)
}

/// Extract text from a lyrics container element.
fn extract_container_text(element: &scraper::ElementRef) -> String {
    let mut result = String::new();

    for node in element.children() {
        match node.value() {
            scraper::node::Node::Text(text) => {
                result.push_str(text.text.as_ref());
            }
            scraper::node::Node::Element(el) => {
                let tag = el.name.local.as_ref();
                match tag {
                    "br" => result.push('\n'),
                    "a" | "i" | "b" | "span" => {
                        if let Some(child) = scraper::ElementRef::wrap(node) {
                            result.push_str(&extract_container_text(&child));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    result
}

/// A single annotation with its lyric fragment and explanation body.
#[derive(Debug, Clone)]
pub struct Annotation {
    pub fragment: String,
    pub body: String,
}

/// Extract annotations from Genius page by parsing `__INITIAL_STATE__` JSON.
/// This gives ALL annotations for the song without API token or pagination.
pub fn extract_annotations(html: &str) -> Result<Vec<Annotation>, String> {
    let json = extract_initial_state(html).ok_or("No __INITIAL_STATE__ found on page")?;

    let annotations_map = match json.pointer("/annotations") {
        Some(Value::Object(map)) => map,
        _ => return Err("No annotations in initial state".to_string()),
    };

    let mut annotations = Vec::new();

    for (_key, val) in annotations_map {
        let fragment = val
            .get("fragment")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = val
            .pointer("/body/dom")
            .and_then(|dom| extract_text_from_dom(dom))
            .unwrap_or_default();

        if !fragment.is_empty() && !body.is_empty() {
            annotations.push(Annotation { fragment, body });
        }
    }

    if annotations.is_empty() {
        return Err("No annotation data could be extracted".to_string());
    }

    Ok(annotations)
}

/// Find and parse `window.__INITIAL_STATE__` JSON from script tag.
fn extract_initial_state(html: &str) -> Option<Value> {
    let marker = "window.__INITIAL_STATE__ = ";
    let start = html.find(marker)?;
    let start = start + marker.len();
    let rest = &html[start..];

    // Find the matching semicolon after the JSON object
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    let end = rest
        .char_indices()
        .find(|&(_, c)| {
            if escaped {
                escaped = false;
                return false;
            }
            match c {
                '\\' => escaped = true,
                '"' if !escaped => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                _ => {}
            }
            false
        })
        .map(|(i, _)| i + 1);

    let json_str = match end {
        Some(end) => &rest[..end],
        None => return None,
    };

    serde_json::from_str(json_str).ok()
}

/// Extract text from a Genius DOM tree structure (used for annotation bodies).
fn extract_text_from_dom(dom: &Value) -> Option<String> {
    match dom {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => {
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
                    Some(texts.join(" "))
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Clean raw lyrics text.
fn clean_lyrics(raw: &str) -> String {
    let decoded = decode_html_entities(raw);
    let mut lines: Vec<String> = Vec::new();

    for line in decoded.lines() {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains("You might also like") || trimmed.contains("Contributors")
            || trimmed.contains("Embed") || trimmed.contains("Cancel")
            || trimmed.contains("How to Format Lyrics")
        {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !lines.is_empty() {
            // Blank line before section header = spacing between sections
            lines.push(String::new());
        }
        lines.push(trimmed);
    }

    lines.join("\n")
}

/// Decode common HTML entities.
fn decode_html_entities(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '&' {
            if let Some(end) = chars[i..].iter().position(|&c| c == ';') {
                let entity: String = chars[i + 1..i + end].iter().collect();
                let decoded = match entity.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "#x27" => "'",
                    "#x2019" => "'",
                    "#x2018" => "'",
                    "#x2014" => "--",
                    "#x2013" => "-",
                    "#x201C" => "\"",
                    "#x201D" => "\"",
                    "#x200B" => "",
                    _ => {
                        if entity.starts_with('#') {
                            if let Ok(code) = entity[1..].parse::<u32>() {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                    i += end + 1;
                                    continue;
                                }
                            }
                        }
                        &s[i..=i + end]
                    }
                };
                result.push_str(decoded);
                i += end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_lyrics() {
        let html = r#"<html><body>
            <div data-lyrics-container="true">
                [Verse 1]<br>
                First line of verse<br>
                Second line<br>
                <br>
                [Chorus]<br>
                Chorus line one<br>
                Chorus line two<br>
            </div>
        </body></html>"#;

        let lyrics = extract_lyrics(html).unwrap();
        assert!(lyrics.contains("[Verse 1]"));
        assert!(lyrics.contains("First line of verse"));
        assert!(lyrics.contains("[Chorus]"));
        assert!(lyrics.contains("Chorus line one"));
    }

    #[test]
    fn test_extract_with_annotation_links() {
        let html = r#"<html><body>
            <div data-lyrics-container="true">
                [Verse 1]<br>
                <a href="/123" class="referent">First</a> line with <a href="/456">annotation</a><br>
                Second line<br>
            </div>
        </body></html>"#;

        let lyrics = extract_lyrics(html).unwrap();
        assert!(lyrics.contains("First line with annotation"));
    }

    #[test]
    fn test_extract_no_container() {
        let html = "<html><body><p>No lyrics here</p></body></html>";
        let result = extract_lyrics(html);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_html_entities() {
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&quot;"), "\"");
        assert_eq!(decode_html_entities("&#x27;"), "'");
        assert_eq!(decode_html_entities("&lt;br&gt;"), "<br>");
    }

    #[test]
    fn test_extract_initial_state() {
        let html = r#"<html><script>window.__INITIAL_STATE__ = {"annotations":{"123":{"fragment":"test","body":{"dom":{"children":["hello"]}}}}};;</script></html>"#;
        let state = extract_initial_state(html);
        assert!(state.is_some());
        let annotations = extract_annotations(html).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].fragment, "test");
        assert_eq!(annotations[0].body, "hello");
    }

    #[test]
    fn test_extract_annotations_no_state() {
        let html = "<html><body>No state here</body></html>";
        let result = extract_annotations(html);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_initial_state_with_escaped_chars() {
        let html = r#"<html><script>window.__INITIAL_STATE__ = {"key":"value with \"quotes\""};;</script></html>"#;
        let state = extract_initial_state(html);
        assert!(state.is_some());
        assert_eq!(state.unwrap()["key"].as_str().unwrap(), "value with \"quotes\"");
    }
}
