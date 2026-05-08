use std::{
    fs,
    path::{Path, PathBuf},
};

const PRICES_URL: &str = "https://api.warframestat.us/wfinfo/prices/";
const FILTERED_ITEMS_URL: &str = "https://api.warframestat.us/wfinfo/filtered_items/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DataCacheRefresh {
    pub(crate) prices_path: PathBuf,
    pub(crate) filtered_items_path: PathBuf,
}

pub(crate) async fn refresh_wfinfo_cache(cache_dir: PathBuf) -> Result<DataCacheRefresh, String> {
    fs::create_dir_all(&cache_dir).map_err(|err| {
        format!(
            "could not create data cache directory {}: {err}",
            cache_dir.display()
        )
    })?;

    let prices_path = download_payload(PRICES_URL, &cache_dir, "prices.json").await?;
    let filtered_items_path =
        download_payload(FILTERED_ITEMS_URL, &cache_dir, "filtered_items.json").await?;

    Ok(DataCacheRefresh {
        prices_path,
        filtered_items_path,
    })
}

async fn download_payload(url: &str, directory: &Path, filename: &str) -> Result<PathBuf, String> {
    log::debug!("downloading WFInfo data payload {url}");
    let response = reqwest::get(url)
        .await
        .map_err(|err| format!("could not download {url}: {err}"))?;
    let status = response.status();

    if !status.is_success() {
        return Err(format!("could not download {url}: HTTP {status}"));
    }

    let body = response
        .text()
        .await
        .map_err(|err| format!("could not read response body from {url}: {err}"))?;
    let path = directory.join(filename);

    write_atomically(&path, body.as_bytes())?;
    log::debug!("cached WFInfo data payload {url} at {}", path.display());

    Ok(path)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::write_atomically;

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
}
