use super::models::{GhAsset, GhRelease, ReleaseAssetMatch};

fn current_host_arch_token() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    }
}

fn asset_matches_arch_preference(name_lower: &str, arch_token: &str) -> u8 {
    let mentions_other = (arch_token != "x64" && name_lower.contains("x64"))
        || (arch_token != "x86" && name_lower.contains("x86"))
        || (arch_token != "arm64" && name_lower.contains("arm64"));
    if name_lower.contains(arch_token) {
        3
    } else if name_lower.contains("universal")
        || (!mentions_other && name_lower.ends_with("-windows-setup.exe"))
    {
        2
    } else if !mentions_other {
        1
    } else {
        0
    }
}

pub(crate) fn find_best_release_match(
    releases: &[GhRelease],
    asset_prefix: &str,
    prefer_portable_zip: bool,
) -> Option<ReleaseAssetMatch> {
    let prefix_lower = asset_prefix.to_ascii_lowercase();
    let arch_tok = current_host_arch_token();
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
                l.starts_with(&prefix_lower)
                    && (l.ends_with(".zip") || l.ends_with(".exe"))
                    && asset_matches_arch_preference(&l, arch_tok) > 0
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }

        let chosen = candidates
            .iter()
            .max_by_key(|a| {
                let l = a.name.to_ascii_lowercase();
                let kind_score = if prefer_portable_zip {
                    if l.contains("portable") && l.ends_with(".zip") {
                        3u8
                    } else if l.ends_with(".zip") {
                        2u8
                    } else {
                        1u8
                    }
                } else if (l.contains("setup") || l.contains("installer")) && l.ends_with(".exe") {
                    3u8
                } else if l.ends_with(".exe") {
                    2u8
                } else {
                    1u8
                };
                let arch_score = asset_matches_arch_preference(&l, arch_tok);
                (kind_score, arch_score)
            })
            .copied()?;

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

    #[test]
    fn test_driver_arch_and_universal_matching() {
        assert_eq!(
            asset_matches_arch_preference("nfc-bridge-driver-0.2.0-windows-x86-setup.exe", "x86"),
            3
        );
        assert_eq!(
            asset_matches_arch_preference(
                "nfc-bridge-driver-0.2.0-windows-setup.exe",
                "x86"
            ),
            2
        );
        assert_eq!(
            asset_matches_arch_preference("nfc-bridge-driver-0.2.0-windows-arm64-setup.exe", "x64"),
            0
        );

        let only_uni_and_other = vec![GhRelease {
            tag_name: "v0.2.1".to_string(),
            draft: false,
            assets: vec![
                GhAsset {
                    name: "nfc-bridge-driver-0.2.1-windows-arm64-setup.exe".to_string(),
                    browser_download_url: "https://example.com/drv-arm64.exe".to_string(),
                    size: 100,
                },
                GhAsset {
                    name: "nfc-bridge-driver-0.2.1-windows-setup.exe".to_string(),
                    browser_download_url: "https://example.com/drv-uni.exe".to_string(),
                    size: 300,
                },
            ],
        }];
        let picked = find_best_release_match(&only_uni_and_other, "nfc-bridge-driver-", false)
            .expect("should pick universal or host asset");
        if current_host_arch_token() == "arm64" {
            assert_eq!(
                picked.asset_name,
                "nfc-bridge-driver-0.2.1-windows-arm64-setup.exe"
            );
        } else {
            assert_eq!(
                picked.asset_name,
                "nfc-bridge-driver-0.2.1-windows-setup.exe"
            );
        }
    }
}
