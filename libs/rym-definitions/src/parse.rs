use crate::scrape::{DescriptorEntry, GenreEntry};
use scraper::{Html, Selector};

/// Parse genre entries from the `/genres/` page HTML.
pub fn parse_genres(html: &str) -> Result<Vec<GenreEntry>, String> {
    let doc = Html::parse_document(html);
    let selector = Selector::parse(".genre_card, .genre, .genre-list-item, .genreListItem")
        .map_err(|e| format!("Selector parse error: {}", e))?;

    let mut genres = Vec::new();
    for element in doc.select(&selector) {
        let name = extract_genre_name(&element);
        let description = extract_genre_description(&element);
        if !name.is_empty() {
            genres.push(GenreEntry { name, description });
        }
    }

    if genres.is_empty() {
        // Fallback: try to find any heading-looking elements
        // Will be refined once we see the actual HTML
        Err("No genres found — inspect HTML structure".to_string())
    } else {
        Ok(genres)
    }
}

/// Parse descriptor entries from the `/descriptors/` page HTML.
pub fn parse_descriptors(html: &str) -> Result<Vec<DescriptorEntry>, String> {
    let doc = Html::parse_document(html);
    let selector = Selector::parse(".descriptor_card, .descriptor, .descriptor-list-item, .descriptorListItem")
        .map_err(|e| format!("Selector parse error: {}", e))?;

    let mut descriptors = Vec::new();
    for element in doc.select(&selector) {
        let name = extract_descriptor_name(&element);
        let explanation = extract_descriptor_explanation(&element);
        if !name.is_empty() {
            descriptors.push(DescriptorEntry { name, explanation });
        }
    }

    if descriptors.is_empty() {
        Err("No descriptors found — inspect HTML structure".to_string())
    } else {
        Ok(descriptors)
    }
}

fn extract_genre_name(element: &scraper::ElementRef) -> String {
    // Try multiple possible selectors for the name element
    for sel_str in &[".genre_card_name a", ".genre_card_name span", ".genre_name a", ".genre_name span", "h3 a", "h3", "a strong", "strong"] {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = element.select(&sel).next() {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    // Try the element's own text (excluding children)
    element.text().collect::<String>().trim().to_string()
}

fn extract_genre_description(element: &scraper::ElementRef) -> String {
    for sel_str in &[".genre_card_desc", ".genre_desc", ".genre-description", "p", ".description", ".desc"] {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = element.select(&sel).next() {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    String::new()
}

fn extract_descriptor_name(element: &scraper::ElementRef) -> String {
    for sel_str in &[".descriptor_card_name a", ".descriptor_card_name span", ".descriptor_name a", ".descriptor_name", "h3 a", "h3", "a strong"] {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = element.select(&sel).next() {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    element.text().collect::<String>().trim().to_string()
}

fn extract_descriptor_explanation(element: &scraper::ElementRef) -> String {
    for sel_str in &[".descriptor_card_exp", ".descriptor_exp", ".descriptor-explanation", "p", ".explanation", ".exp"] {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = element.select(&sel).next() {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    String::new()
}
