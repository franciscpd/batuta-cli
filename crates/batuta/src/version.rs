use semver::Version;

pub const MIN_COMPOZY_VERSION: &str = "v0.3.0-beta.16";

pub fn check(version: Option<&str>) -> Option<String> {
    let value = version.unwrap_or("").trim();
    if value == "dev" {
        return Some(unverified("dev"));
    }
    let parsed = Version::parse(value.strip_prefix('v').unwrap_or(value));
    let floor =
        Version::parse(MIN_COMPOZY_VERSION.strip_prefix('v').unwrap()).expect("valid floor");
    match parsed {
        Ok(version) if version >= floor => None,
        Ok(_) => Some(format!(
            "daemon version {value} is below supported floor {MIN_COMPOZY_VERSION}"
        )),
        Err(_) => Some(unverified("unknown")),
    }
}

fn unverified(version: &str) -> String {
    format!("daemon version {version} — compatibility unverified (floor {MIN_COMPOZY_VERSION})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ut_070_dev_version_warns() {
        assert_eq!(check(Some("dev")).unwrap(), unverified("dev"));
    }
    #[test]
    fn ut_071_old_version_warns_and_floor_passes() {
        assert!(check(Some("v0.2.9")).unwrap().contains("v0.2.9"));
        assert_eq!(check(Some("v0.3.0-beta.16")), None);
        assert_eq!(check(Some("v0.4.0")), None);
    }
    #[test]
    fn ut_072_git_suffix_is_compatible() {
        assert_eq!(check(Some("v0.3.0-beta.16-9-ga35eda6d")), None);
    }
    #[test]
    fn ut_073_empty_or_invalid_is_unknown() {
        assert!(check(Some("")).unwrap().contains("unknown"));
        assert!(check(Some("garbage")).unwrap().contains("unknown"));
    }
}
