pub fn format_relative_time(secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = (now - secs).max(0);

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!(
            "{} {} ago",
            mins,
            if mins == 1 { "minute" } else { "minutes" }
        )
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!(
            "{} {} ago",
            hours,
            if hours == 1 { "hour" } else { "hours" }
        )
    } else if diff < 604800 {
        let days = diff / 86400;
        format!("{} {} ago", days, if days == 1 { "day" } else { "days" })
    } else if diff < 2592000 {
        let weeks = diff / 604800;
        format!(
            "{} {} ago",
            weeks,
            if weeks == 1 { "week" } else { "weeks" }
        )
    } else if diff < 31536000 {
        let months = diff / 2592000;
        format!(
            "{} {} ago",
            months,
            if months == 1 { "month" } else { "months" }
        )
    } else {
        let years = diff / 31536000;
        format!(
            "{} {} ago",
            years,
            if years == 1 { "year" } else { "years" }
        )
    }
}

fn normalize_github_url(url: &str) -> Option<String> {
    let input = url.trim();
    if input.starts_with("git@github.com:") {
        let path = input.strip_prefix("git@github.com:")?;
        let path = path.strip_suffix(".git").unwrap_or(path);
        let path = path.trim_matches('/');
        let mut parts = path.split('/');
        let org = parts.next()?;
        let repo = parts.next()?;
        if !org.is_empty() && !repo.is_empty() {
            return Some(format!("https://github.com/{}/{}", org, repo));
        }
    } else if let Ok(parsed) = url::Url::parse(input) {
        if parsed.host_str() == Some("github.com") {
            let path = parsed.path().trim_matches('/');
            let path = path.strip_suffix(".git").unwrap_or(path);
            let mut parts = path.split('/');
            let org = parts.next()?;
            let repo = parts.next()?;
            if !org.is_empty() && !repo.is_empty() {
                return Some(format!("https://github.com/{}/{}", org, repo));
            }
        }
    }
    None
}

pub fn is_github_url(url: &str) -> bool {
    normalize_github_url(url).is_some()
}

pub fn github_links(url: &str) -> Option<(String, String, String)> {
    let base = normalize_github_url(url)?;
    Some((
        base.clone(),
        format!("{}/issues", base),
        format!("{}/pulls", base),
    ))
}

pub fn parse_tag_version(tag: &str) -> (u64, u64, u64) {
    let stripped = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = stripped.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_relative_time_just_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_relative_time(now), "just now");
    }

    #[test]
    fn test_format_relative_time_minutes() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let five_min_ago = now - 300;
        assert_eq!(format_relative_time(five_min_ago), "5 minutes ago");
    }

    #[test]
    fn test_format_relative_time_hours() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let three_hours_ago = now - 10800;
        assert_eq!(format_relative_time(three_hours_ago), "3 hours ago");
    }

    #[test]
    fn test_format_relative_time_days() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let two_days_ago = now - 172800;
        assert_eq!(format_relative_time(two_days_ago), "2 days ago");
    }

    #[test]
    fn test_format_relative_time_months() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let seven_months_ago = now - 18144000;
        assert_eq!(format_relative_time(seven_months_ago), "7 months ago");
    }

    #[test]
    fn test_is_github_url_true() {
        assert!(is_github_url("https://github.com/user/repo"));
        assert!(is_github_url("git@github.com:user/repo.git"));
    }

    #[test]
    fn test_is_github_url_false() {
        assert!(!is_github_url("https://gitlab.com/user/repo"));
        assert!(!is_github_url("https://bitbucket.org/user/repo"));
    }

    #[test]
    fn test_github_links_returns_some() {
        let links = github_links("https://github.com/user/repo");
        assert!(links.is_some());
        let (base, issues, pulls) = links.unwrap();
        assert_eq!(base, "https://github.com/user/repo");
        assert_eq!(issues, "https://github.com/user/repo/issues");
        assert_eq!(pulls, "https://github.com/user/repo/pulls");
    }

    #[test]
    fn test_github_links_strips_git_extension() {
        let links = github_links("https://github.com/user/repo.git");
        let (base, _, _) = links.unwrap();
        assert_eq!(base, "https://github.com/user/repo");
    }

    #[test]
    fn test_github_links_returns_none_for_non_github() {
        assert!(github_links("https://gitlab.com/user/repo").is_none());
    }

    #[test]
    fn test_normalize_github_url_robust() {
        assert_eq!(
            normalize_github_url("https://github.com/user/repo"),
            Some("https://github.com/user/repo".to_string())
        );
        assert_eq!(
            normalize_github_url("git@github.com:user/repo.git"),
            Some("https://github.com/user/repo".to_string())
        );
        assert_eq!(
            normalize_github_url("git@github.com:user/repo/"),
            Some("https://github.com/user/repo".to_string())
        );
        assert_eq!(
            normalize_github_url("https://github.com/user/repo.git"),
            Some("https://github.com/user/repo".to_string())
        );
        assert_eq!(normalize_github_url("https://github.com/user"), None);
        assert_eq!(normalize_github_url("github.com/user/repo"), None);
    }
}
