pub mod repo_manager_body;
pub mod repo_manager_sidebar;

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

pub fn ownership_badge_text(owned: Option<bool>) -> &'static str {
    match owned {
        Some(true) => "Owned by you",
        Some(false) => "External repo",
        None => "Ownership unknown",
    }
}

pub fn parse_tag_version(tag: &str) -> (u64, u64, u64) {
    let stripped = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = stripped.split('.');

    let parse_part = |s: &str| -> u64 {
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok().unwrap_or(0)
    };

    let major = parts.next().map(parse_part).unwrap_or(0);
    let minor = parts.next().map(parse_part).unwrap_or(0);
    let patch = parts.next().map(parse_part).unwrap_or(0);
    (major, minor, patch)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoOwnershipFilterLabel {
    All,
    Owned,
    External,
}

impl RepoOwnershipFilterLabel {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All repos",
            Self::Owned => "Owned repos",
            Self::External => "External repos",
        }
    }
}
