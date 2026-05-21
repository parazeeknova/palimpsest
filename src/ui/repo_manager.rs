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

pub fn is_github_url(url: &str) -> bool {
    url.contains("github.com")
}

pub fn github_links(url: &str) -> Option<(String, String, String)> {
    if !is_github_url(url) {
        return None;
    }
    let base = url.trim_end_matches(".git");
    Some((
        base.to_string(),
        format!("{}/issues", base),
        format!("{}/pulls", base),
    ))
}
