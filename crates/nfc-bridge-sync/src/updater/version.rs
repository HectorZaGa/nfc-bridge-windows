use super::models::{GhAsset, GhRelease, ReleaseAssetMatch};

pub(crate) fn find_best_release_match(
    releases: &[GhRelease],
    asset_prefix: &str,
    prefer_portable_zip: bool,
) -> Option<ReleaseAssetMatch> {
    let prefix_lower = asset_prefix.to_ascii_lowercase();
    let mut best: Option<ReleaseAssetMatch> = None;

    for rel in releases {
        if rel.draft {
            continue;
        }
        let candidates: Vec<&GhAsset> = rel
            .assets
            .iter()
            .filter(|a| {
                let l = a.name.to_ascii_lowercase();
                l.starts_with(&prefix_lower) && (l.ends_with(".zip") || l.ends_with(".exe"))
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }

        let chosen = if prefer_portable_zip {
            candidates
                .iter()
                .find(|a| {
                    let l = a.name.to_ascii_lowercase();
                    l.contains("portable") && l.ends_with(".zip")
                })
                .or_else(|| candidates.iter().find(|a| a.name.to_ascii_lowercase().ends_with(".zip")))
                .or_else(|| candidates.first())
                .copied()?
        } else {
            candidates
                .iter()
                .find(|a| {
                    let l = a.name.to_ascii_lowercase();
                    (l.contains("setup") || l.contains("installer")) && l.ends_with(".exe")
                })
                .or_else(|| candidates.iter().find(|a| a.name.to_ascii_lowercase().ends_with(".exe")))
                .or_else(|| candidates.first())
                .copied()?
        };

        let ver = extract_semver_from_names(&chosen.name, &rel.tag_name)?;
        let candidate = ReleaseAssetMatch {
            version: ver,
            tag_name: rel.tag_name.clone(),
            asset_name: chosen.name.clone(),
            download_url: chosen.browser_download_url.clone(),
            size_bytes: chosen.size,
        };
        if best
            .as_ref()
            .is_none_or(|cur| compare_semver(&candidate.version, &cur.version) > 0)
        {
            best = Some(candidate);
        }
    }
    best
}

pub fn normalize_base_version(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    for token in trimmed.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = token.split('.').filter(|s| !s.is_empty()).collect();
        if parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
            return token.to_string();
        }
    }
    trimmed.to_string()
}

fn extract_semver_from_names(asset_name: &str, tag_name: &str) -> Option<String> {
    extract_triple_digits(asset_name).or_else(|| extract_triple_digits(tag_name))
}

fn extract_triple_digits(text: &str) -> Option<String> {
    for chunk in text.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let parts: Vec<&str> = chunk.split('.').collect();
        if parts.len() == 3
            && !parts.iter().any(|p| p.is_empty())
            && parts.iter().all(|p| p.parse::<u32>().is_ok())
        {
            return Some(chunk.to_string());
        }
    }
    None
}

pub fn compare_semver(a: &str, b: &str) -> i32 {
    let pa = parse_semver_tuple(a);
    let pb = parse_semver_tuple(b);
    if pa > pb {
        1
    } else if pa < pb {
        -1
    } else {
        0
    }
}

fn parse_semver_tuple(raw: &str) -> (u32, u32, u32) {
    let norm = normalize_base_version(raw);
    let mut it = norm.split('.').map(|s| s.parse::<u32>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_comparison() {
        assert_eq!(compare_semver("0.1.24", "0.1.23"), 1);
        assert_eq!(compare_semver("v1.0.0", "0.9.9"), 1);
        assert_eq!(compare_semver("0.1.23", "0.1.23"), 0);
        assert_eq!(compare_semver("0.1.22", "0.1.23"), -1);
    }

    #[test]
    fn test_variant_isolation_matching() {
        let releases = vec![GhRelease {
            tag_name: "v0.2.0".to_string(),
            draft: false,
            assets: vec![
                GhAsset {
                    name: "nfc-bridge-tools-0.2.0-windows-x64-portable.zip".to_string(),
                    browser_download_url: "https://example.com/tools.zip".to_string(),
                    size: 100,
                },
                GhAsset {
                    name: "nfc-bridge-sync-0.2.0-windows-x64-setup.exe".to_string(),
                    browser_download_url: "https://example.com/sync-setup.exe".to_string(),
                    size: 100,
                },
                GhAsset {
                    name: "nfc-bridge-sync-0.2.0-windows-x64-portable.zip".to_string(),
                    browser_download_url: "https://example.com/sync-portable.zip".to_string(),
                    size: 100,
                },
            ],
        }];

        let sync_portable = find_best_release_match(&releases, "nfc-bridge-sync-", true).unwrap();
        assert_eq!(
            sync_portable.asset_name,
            "nfc-bridge-sync-0.2.0-windows-x64-portable.zip"
        );
        assert_eq!(sync_portable.version, "0.2.0");

        let sync_setup = find_best_release_match(&releases, "nfc-bridge-sync-", false).unwrap();
        assert_eq!(
            sync_setup.asset_name,
            "nfc-bridge-sync-0.2.0-windows-x64-setup.exe"
        );

        let tools_setup = find_best_release_match(&releases, "nfc-bridge-tools-", false).unwrap();
        assert_eq!(
            tools_setup.asset_name,
            "nfc-bridge-tools-0.2.0-windows-x64-portable.zip"
        );
    }
}
