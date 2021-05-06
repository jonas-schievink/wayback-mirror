//! Wayback machine CDX API client.

use crate::Result;
use async_std::{prelude::*, stream};
use futures::StreamExt;
use serde::Serialize;
use surf::Url;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
enum MatchType {
    Exact,
    Prefix,
    Host,
    Domain,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Output {
    Json,
}

/// URL query parameters
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Query<'a> {
    url: &'a str,
    #[serde(rename = "fl")]
    fields: &'a str,
    match_type: MatchType,
    gzip: bool,
    output: Option<Output>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    #[serde(rename = "filter")]
    filters: &'a [&'a str],
    #[serde(skip_serializing_if = "Option::is_none")]
    collapse: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<usize>,
}

#[derive(Debug)]
pub struct Entry {
    pub timestamp: String,
    pub original: Url,
}

pub fn query_snapshots(
    url: &str,
    pages: usize,
    concurrency: usize,
) -> impl Stream<Item = Result<Vec<Entry>>> + '_ {
    log::info!("querying snapshots of URL {}", url);
    stream::from_iter((0..pages).map(move |page| query_snapshot_page(url, page)))
        .buffer_unordered(concurrency)
}

async fn query_snapshot_page(url_str: &str, page: usize) -> Result<Vec<Entry>> {
    let url = Url::parse(url_str)?;
    let resp = surf::get("https://web.archive.org/cdx/search/cdx")
        .query(&Query {
            url: url_str,
            fields: "timestamp,original",
            match_type: MatchType::Prefix,
            gzip: false,
            output: Some(Output::Json),
            filters: &["statuscode:200"],
            collapse: Some("digest"),
            page: Some(page),
        })
        .unwrap()
        .recv_string()
        .await?;

    // The server sends back an empty response when there are no results on this page.
    if resp.is_empty() {
        return Ok(Vec::new());
    }

    let resp: Vec<Vec<String>> = serde_json::from_str(&resp)?;

    let entries = resp
        .into_iter()
        .skip(1)
        .map(|raw| match &*raw {
            [timestamp, original] => {
                let original = Url::parse(original)?;

                // Ignore port and scheme in this comparison
                if url.host_str() == original.host_str() {
                    Ok(Some(Entry {
                        timestamp: timestamp.clone(),
                        original,
                    }))
                } else {
                    log::warn!("skipping URL {} (doesn't match query URL)", original);
                    Ok(None)
                }
            }
            _ => Err(format!("malformed response (expected 2 fields)").into()),
        })
        .filter_map(|res| res.transpose())
        .collect::<Result<Vec<_>>>()?;

    Ok(entries)
}
