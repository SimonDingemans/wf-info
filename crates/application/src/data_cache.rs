use std::{
    fs,
    path::{Path, PathBuf},
};

const PRICES_PAYLOAD: RemotePayload = RemotePayload {
    label: "WFInfo prices",
    url: "https://api.warframestat.us/wfinfo/prices/",
    filename: "prices.json",
    validate: validate_prices_payload,
};
const FILTERED_ITEMS_PAYLOAD: RemotePayload = RemotePayload {
    label: "WFInfo filtered items",
    url: "https://api.warframestat.us/wfinfo/filtered_items/",
    filename: "filtered_items.json",
    validate: validate_filtered_items_payload,
};
const WARFRAME_MARKET_ITEMS_PAYLOAD: RemotePayload = RemotePayload {
    label: "warframe.market item metadata",
    url: "https://api.warframe.market/v2/items",
    filename: "warframe_market_items.json",
    validate: validate_warframe_market_items_payload,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataCacheRefresh {
    pub(crate) prices: DataPayloadRefresh,
    pub(crate) filtered_items: DataPayloadRefresh,
    pub(crate) warframe_market_items: DataPayloadRefresh,
}

impl DataCacheRefresh {
    pub(crate) fn remote_count(&self) -> usize {
        self.payloads()
            .into_iter()
            .filter(|(_, payload)| payload.source == DataCacheSource::Remote)
            .count()
    }

    pub(crate) fn local_fallback_count(&self) -> usize {
        self.payloads()
            .into_iter()
            .filter(|(_, payload)| payload.source == DataCacheSource::LocalFallback)
            .count()
    }

    pub(crate) fn payloads(&self) -> [(&'static str, &DataPayloadRefresh); 3] {
        [
            ("Prices", &self.prices),
            ("Filtered items", &self.filtered_items),
            ("warframe.market items", &self.warframe_market_items),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataPayloadRefresh {
    pub(crate) path: PathBuf,
    pub(crate) source: DataCacheSource,
}

impl DataPayloadRefresh {
    fn remote(path: PathBuf) -> Self {
        Self {
            path,
            source: DataCacheSource::Remote,
        }
    }

    fn local_fallback(path: PathBuf) -> Self {
        Self {
            path,
            source: DataCacheSource::LocalFallback,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataCacheSource {
    Remote,
    LocalFallback,
}

impl DataCacheSource {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Remote => "remote",
            Self::LocalFallback => "local fallback",
        }
    }
}

#[derive(Clone, Copy)]
struct RemotePayload {
    label: &'static str,
    url: &'static str,
    filename: &'static str,
    validate: fn(&str) -> Result<(), String>,
}

pub(crate) async fn refresh_wfinfo_cache(cache_dir: PathBuf) -> Result<DataCacheRefresh, String> {
    fs::create_dir_all(&cache_dir).map_err(|err| {
        format!(
            "could not create data cache directory {}: {err}",
            cache_dir.display()
        )
    })?;

    let client = reqwest::Client::new();
    let prices = refresh_payload(&client, &cache_dir, PRICES_PAYLOAD).await?;
    let filtered_items = refresh_payload(&client, &cache_dir, FILTERED_ITEMS_PAYLOAD).await?;
    let warframe_market_items =
        refresh_payload(&client, &cache_dir, WARFRAME_MARKET_ITEMS_PAYLOAD).await?;

    Ok(DataCacheRefresh {
        prices,
        filtered_items,
        warframe_market_items,
    })
}

async fn refresh_payload(
    client: &reqwest::Client,
    directory: &Path,
    payload: RemotePayload,
) -> Result<DataPayloadRefresh, String> {
    let path = directory.join(payload.filename);
    let download = download_payload_body(client, payload).await;

    finish_payload_refresh(payload, &path, download)
}

async fn download_payload_body(
    client: &reqwest::Client,
    payload: RemotePayload,
) -> Result<String, String> {
    log::debug!(
        "downloading data payload {} from {}",
        payload.label,
        payload.url
    );
    let response = client
        .get(payload.url)
        .header(reqwest::header::USER_AGENT, "wf-info")
        .header("Language", "en")
        .send()
        .await
        .map_err(|err| format!("could not download {}: {err}", payload.url))?;
    let status = response.status();

    if !status.is_success() {
        return Err(format!("could not download {}: HTTP {status}", payload.url));
    }

    response
        .text()
        .await
        .map_err(|err| format!("could not read response body from {}: {err}", payload.url))
}

fn finish_payload_refresh(
    payload: RemotePayload,
    path: &Path,
    download: Result<String, String>,
) -> Result<DataPayloadRefresh, String> {
    match download {
        Ok(body) => cache_remote_payload(payload, path, &body)
            .or_else(|err| use_local_fallback(payload, path, err)),
        Err(err) => use_local_fallback(payload, path, err),
    }
}

fn cache_remote_payload(
    payload: RemotePayload,
    path: &Path,
    body: &str,
) -> Result<DataPayloadRefresh, String> {
    (payload.validate)(body)
        .map_err(|err| format!("downloaded {} payload is not usable: {err}", payload.label))?;
    write_atomically(&path, body.as_bytes())?;
    log::debug!(
        "cached data payload {} from {} at {}",
        payload.label,
        payload.url,
        path.display()
    );

    Ok(DataPayloadRefresh::remote(path.to_path_buf()))
}

fn use_local_fallback(
    payload: RemotePayload,
    path: &Path,
    refresh_error: String,
) -> Result<DataPayloadRefresh, String> {
    let cached_body = fs::read_to_string(path).map_err(|read_error| {
        format!(
            "could not refresh {} and no usable local cache exists at {}: {refresh_error}; local read failed: {read_error}",
            payload.label,
            path.display()
        )
    })?;

    (payload.validate)(&cached_body).map_err(|validation_error| {
        format!(
            "could not refresh {} and local cache at {} is not usable: {refresh_error}; local validation failed: {validation_error}",
            payload.label,
            path.display()
        )
    })?;

    log::warn!(
        "using local fallback cache for {} at {} after refresh failed: {}",
        payload.label,
        path.display(),
        refresh_error
    );

    Ok(DataPayloadRefresh::local_fallback(path.to_path_buf()))
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary_path = path.with_extension("json.tmp");

    fs::write(&temporary_path, contents)
        .map_err(|err| format!("could not write {}: {err}", temporary_path.display()))?;
    fs::rename(&temporary_path, path).map_err(|err| {
        format!(
            "could not replace {} with {}: {err}",
            path.display(),
            temporary_path.display()
        )
    })
}

fn validate_prices_payload(body: &str) -> Result<(), String> {
    serde_json::from_str::<Vec<serde_json::Value>>(body)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn validate_filtered_items_payload(body: &str) -> Result<(), String> {
    let payload = serde_json::from_str::<serde_json::Value>(body).map_err(|err| err.to_string())?;

    if payload
        .get("eqmt")
        .is_some_and(serde_json::Value::is_object)
    {
        Ok(())
    } else {
        Err("expected an object with an eqmt map".to_owned())
    }
}

fn validate_warframe_market_items_payload(body: &str) -> Result<(), String> {
    let payload = serde_json::from_str::<serde_json::Value>(body).map_err(|err| err.to_string())?;
    let items = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "expected an object with a data array".to_owned())?;
    let has_item_slug = items.iter().any(|item| {
        item.get("slug")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|slug| !slug.trim().is_empty())
    });

    if has_item_slug {
        Ok(())
    } else {
        Err("expected at least one item with a slug".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        FILTERED_ITEMS_PAYLOAD, PRICES_PAYLOAD, WARFRAME_MARKET_ITEMS_PAYLOAD,
        cache_remote_payload, finish_payload_refresh, validate_filtered_items_payload,
        validate_prices_payload, validate_warframe_market_items_payload, write_atomically,
    };

    #[test]
    fn write_atomically_replaces_existing_cache_file() {
        let directory =
            std::env::temp_dir().join(format!("wf-info-data-cache-test-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("test cache directory");
        let path = directory.join("prices.json");
        fs::write(&path, "old").expect("old cache file");

        write_atomically(&path, b"new").expect("atomic write");

        assert_eq!(fs::read_to_string(&path).expect("cache file"), "new");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn remote_payloads_are_validated_before_replacing_existing_cache() {
        let directory =
            std::env::temp_dir().join(format!("wf-info-data-cache-invalid-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("test cache directory");
        let path = directory.join("prices.json");
        fs::write(&path, "old").expect("old cache file");

        let result = cache_remote_payload(PRICES_PAYLOAD, &path, r#"{"not":"prices"}"#);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).expect("cache file"), "old");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn refresh_uses_valid_local_fallback_when_download_fails() {
        let directory = std::env::temp_dir().join(format!(
            "wf-info-data-cache-fallback-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("test cache directory");
        let path = directory.join("filtered_items.json");
        fs::write(&path, r#"{"eqmt":{},"ignored_items":{}}"#).expect("cache file");

        let refresh =
            finish_payload_refresh(FILTERED_ITEMS_PAYLOAD, &path, Err("offline".to_owned()))
                .expect("fallback cache");

        assert_eq!(refresh.path, path);
        assert_eq!(refresh.source.label(), "local fallback");

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn refresh_uses_valid_local_fallback_when_remote_payload_is_invalid() {
        let directory = std::env::temp_dir().join(format!(
            "wf-info-data-cache-invalid-fallback-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("test cache directory");
        let path = directory.join("prices.json");
        fs::write(&path, r#"[{"name":"Forma Blueprint","custom_avg":"9.0"}]"#).expect("cache file");

        let refresh =
            finish_payload_refresh(PRICES_PAYLOAD, &path, Ok(r#"{"not":"prices"}"#.to_owned()))
                .expect("fallback cache");

        assert_eq!(refresh.path, path);
        assert_eq!(refresh.source.label(), "local fallback");
        assert_eq!(
            fs::read_to_string(&refresh.path).expect("cache file"),
            r#"[{"name":"Forma Blueprint","custom_avg":"9.0"}]"#
        );

        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn refresh_fails_when_download_and_local_fallback_are_unusable() {
        let directory =
            std::env::temp_dir().join(format!("wf-info-data-cache-missing-{}", std::process::id()));
        let path = directory.join("warframe_market_items.json");

        let err = finish_payload_refresh(
            WARFRAME_MARKET_ITEMS_PAYLOAD,
            &path,
            Err("offline".to_owned()),
        )
        .expect_err("missing fallback should fail");

        assert!(err.contains("no usable local cache exists"));
    }

    #[test]
    fn validators_accept_expected_payload_shapes() {
        validate_prices_payload(r#"[{"name":"Forma Blueprint","custom_avg":"9.0"}]"#)
            .expect("prices");
        validate_filtered_items_payload(r#"{"eqmt":{},"ignored_items":{}}"#)
            .expect("filtered items");
        validate_warframe_market_items_payload(
            r#"{"data":[{"slug":"forma_blueprint","i18n":{"en":{"name":"Forma Blueprint"}}}]}"#,
        )
        .expect("market items");
    }
}
