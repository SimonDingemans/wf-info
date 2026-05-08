use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use thiserror::Error;

const WARFRAME_STEAM_APP_ID: &str = "230410";
const WARFRAME_LOG_RELATIVE_PATH: &[&str] = &["AppData", "Local", "Warframe", "EE.log"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarframeLogDiscovery {
    pub path: PathBuf,
    pub searched: Vec<PathBuf>,
}

#[derive(Debug, Error)]
pub enum WarframeLogDiscoveryError {
    #[error("could not find Warframe EE.log in known Steam/Proton locations")]
    NotFound { searched: Vec<PathBuf> },
}

impl WarframeLogDiscoveryError {
    pub fn searched(&self) -> &[PathBuf] {
        match self {
            Self::NotFound { searched } => searched,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::NotFound { searched } if searched.is_empty() => {
                "Could not discover Warframe EE.log. Set warframe.log_path in settings.".to_owned()
            }
            Self::NotFound { searched } => format!(
                "Could not discover Warframe EE.log after checking {} known location(s). Set warframe.log_path in settings.",
                searched.len()
            ),
        }
    }
}

pub fn discover_warframe_log_path() -> Result<WarframeLogDiscovery, WarframeLogDiscoveryError> {
    discover_warframe_log_path_from_candidates(warframe_log_candidates())
}

pub fn discover_warframe_log_path_from_candidates(
    candidates: Vec<PathBuf>,
) -> Result<WarframeLogDiscovery, WarframeLogDiscoveryError> {
    let searched = unique_paths(candidates);

    select_existing_warframe_log_path(&searched)
        .map(|path| WarframeLogDiscovery {
            path,
            searched: searched.clone(),
        })
        .ok_or(WarframeLogDiscoveryError::NotFound { searched })
}

pub fn warframe_log_candidates() -> Vec<PathBuf> {
    let environment = DiscoveryEnvironment::from_process();
    warframe_log_candidates_for_environment(&environment)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DiscoveryEnvironment {
    home: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
    steam_compat_client_install_path: Option<PathBuf>,
}

impl DiscoveryEnvironment {
    fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME").map(PathBuf::from),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            steam_compat_client_install_path: std::env::var_os("STEAM_COMPAT_CLIENT_INSTALL_PATH")
                .map(PathBuf::from),
        }
    }
}

fn warframe_log_candidates_for_environment(environment: &DiscoveryEnvironment) -> Vec<PathBuf> {
    let steam_roots = steam_roots(environment);
    let library_roots = steam_roots
        .iter()
        .flat_map(|root| steam_library_roots(root))
        .collect::<Vec<_>>();

    unique_paths(
        library_roots
            .iter()
            .flat_map(|root| warframe_log_candidates_for_steam_library(root))
            .collect(),
    )
}

fn steam_roots(environment: &DiscoveryEnvironment) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(path) = &environment.steam_compat_client_install_path {
        push_unique_path(&mut roots, path.clone());
    }

    if let Some(path) = &environment.xdg_data_home {
        push_unique_path(&mut roots, path.join("Steam"));
    }

    if let Some(home) = &environment.home {
        push_unique_path(&mut roots, home.join(".local").join("share").join("Steam"));
        push_unique_path(&mut roots, home.join(".steam").join("steam"));
        push_unique_path(&mut roots, home.join(".steam").join("debian-installation"));
        push_unique_path(
            &mut roots,
            home.join(".var")
                .join("app")
                .join("com.valvesoftware.Steam")
                .join(".local")
                .join("share")
                .join("Steam"),
        );
    }

    roots
}

fn steam_library_roots(steam_root: &PathBuf) -> Vec<PathBuf> {
    let mut roots = vec![steam_root.clone()];
    let libraryfolders_path = steam_root.join("steamapps").join("libraryfolders.vdf");

    if let Ok(contents) = fs::read_to_string(&libraryfolders_path) {
        for path in parse_steam_library_paths(&contents) {
            push_unique_path(&mut roots, path);
        }
    }

    roots
}

fn parse_steam_library_paths(contents: &str) -> Vec<PathBuf> {
    contents
        .lines()
        .filter_map(parse_steam_library_path_line)
        .collect()
}

fn parse_steam_library_path_line(line: &str) -> Option<PathBuf> {
    let values = quoted_values(line);

    match values.as_slice() {
        [key, path] if key == "path" => Some(PathBuf::from(path)),
        _ => None,
    }
}

fn quoted_values(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        match character {
            '\\' if in_quote => escaped = true,
            '"' if in_quote => {
                values.push(current.clone());
                current.clear();
                in_quote = false;
            }
            '"' => in_quote = true,
            _ if in_quote => current.push(character),
            _ => {}
        }
    }

    values
}

fn warframe_log_candidates_for_steam_library(library_root: &PathBuf) -> Vec<PathBuf> {
    let prefix = library_root
        .join("steamapps")
        .join("compatdata")
        .join(WARFRAME_STEAM_APP_ID)
        .join("pfx");

    warframe_log_candidates_for_wine_prefix(prefix)
}

fn warframe_log_candidates_for_wine_prefix(prefix: PathBuf) -> Vec<PathBuf> {
    let users_dir = prefix.join("drive_c").join("users");
    let mut users = vec![PathBuf::from("steamuser")];

    if let Some(username) = std::env::var_os("USER") {
        push_unique_path(&mut users, PathBuf::from(username));
    }

    if let Ok(entries) = fs::read_dir(&users_dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                push_unique_path(&mut users, PathBuf::from(entry.file_name()));
            }
        }
    }

    users
        .into_iter()
        .map(|user| {
            WARFRAME_LOG_RELATIVE_PATH
                .iter()
                .fold(users_dir.join(user), |path, segment| path.join(segment))
        })
        .collect()
}

fn select_existing_warframe_log_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .filter_map(|candidate| {
            fs::metadata(candidate)
                .ok()
                .filter(|metadata| metadata.is_file())
                .map(|metadata| {
                    (
                        candidate.clone(),
                        metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    )
                })
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path)
}

fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().fold(Vec::new(), |mut unique, path| {
        push_unique_path(&mut unique, path);
        unique
    })
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{
        DiscoveryEnvironment, discover_warframe_log_path_from_candidates,
        parse_steam_library_paths, warframe_log_candidates_for_environment,
    };

    #[test]
    fn parses_steam_library_paths_from_vdf() {
        let paths = parse_steam_library_paths(
            r#"
"libraryfolders"
{
    "0"
    {
        "path"      "/home/simon/.local/share/Steam"
    }
    "1"
    {
        "path"      "/mnt/games/SteamLibrary"
    }
}
"#,
        );

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/simon/.local/share/Steam"),
                PathBuf::from("/mnt/games/SteamLibrary")
            ]
        );
    }

    #[test]
    fn builds_candidates_from_steam_and_flatpak_roots() {
        let home = PathBuf::from("/home/simon");
        let candidates = warframe_log_candidates_for_environment(&DiscoveryEnvironment {
            home: Some(home.clone()),
            xdg_data_home: None,
            steam_compat_client_install_path: None,
        });

        assert!(candidates.contains(&home.join(".local/share/Steam/steamapps/compatdata/230410/pfx/drive_c/users/steamuser/AppData/Local/Warframe/EE.log")));
        assert!(candidates.contains(&home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/compatdata/230410/pfx/drive_c/users/steamuser/AppData/Local/Warframe/EE.log")));
    }

    #[test]
    fn discovery_selects_existing_candidate() {
        let path = temp_log_path("existing");
        fs::write(&path, "log").expect("fixture log should write");

        let discovery = discover_warframe_log_path_from_candidates(vec![
            PathBuf::from("/missing"),
            path.clone(),
        ])
        .expect("existing candidate should be selected");

        assert_eq!(discovery.path, path);
        assert_eq!(discovery.searched.len(), 2);

        let _ = fs::remove_file(discovery.path);
    }

    #[test]
    fn discovery_reports_searched_candidates_when_not_found() {
        let missing = temp_log_path("missing");

        let err = discover_warframe_log_path_from_candidates(vec![missing.clone()])
            .expect_err("missing candidate should fail discovery");

        assert_eq!(err.searched(), &[missing]);
        assert!(err.user_message().contains("Set warframe.log_path"));
    }

    fn temp_log_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos();

        std::env::temp_dir().join(format!("wf-info-discovery-{name}-{suffix}.log"))
    }
}
