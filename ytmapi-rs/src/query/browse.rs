use super::{PostMethod, PostQuery, Query};
use crate::auth::AuthToken;
use crate::json::Json;
use crate::parse::{ParseFrom, ProcessedResult};
use serde_json::json;
use std::borrow::Cow;

/// Generic browse query by `browseId`. Returns the raw JSON response, which is
/// useful for debugging any YTM page (artist, album, playlist, library section,
/// home, charts, etc.) directly from the CLI.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BrowseQuery {
    browse_id: String,
}

impl BrowseQuery {
    pub fn new(browse_id: impl Into<String>) -> Self {
        Self {
            browse_id: browse_id.into(),
        }
    }
}

impl<A: AuthToken> Query<A> for BrowseQuery {
    type Output = Json;
    type Method = PostMethod;
}

impl PostQuery for BrowseQuery {
    fn header(&self) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::from_iter([("browseId".to_string(), json!(self.browse_id))])
    }
    fn path(&self) -> &str {
        "browse"
    }
    fn params(&self) -> Vec<(&str, Cow<'_, str>)> {
        vec![]
    }
}

impl ParseFrom<BrowseQuery> for Json {
    fn parse_from(p: ProcessedResult<BrowseQuery>) -> crate::Result<Self> {
        Ok(p.json)
    }
}
