pub fn is_link(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with("https://") || t.starts_with("http://")) && t.len() > t.find("://").unwrap_or(0) + 3
}

pub fn is_issue_id(text: &str) -> bool {
    let t = text.trim();
    let Some((project, number)) = t.rsplit_once('-') else {
        return false;
    };
    is_project_key(project) && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
}

pub fn is_project_key(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && text.len() <= 20
}

pub fn workspace_root(workspace: &str) -> Option<String> {
    let w = workspace.trim();
    if !is_link(w) {
        return None;
    }
    let mut root = w.trim_end_matches('/').to_string();
    for cut in ["/issue/", "/issues", "/projects", "/agiles", "/dashboard", "/articles"] {
        if let Some(at) = root.find(cut) {
            root.truncate(at);
        }
    }
    Some(root.trim_end_matches('/').to_string())
}

pub fn script_link(script: &str, workspace: &str) -> Option<String> {
    let s = script.trim();
    if s.is_empty() {
        return None;
    }
    if is_link(s) {
        return Some(s.to_string());
    }
    let root = workspace_root(workspace)?;
    if is_issue_id(s) {
        Some(format!("{root}/issue/{}", s.to_ascii_uppercase()))
    } else if is_project_key(s) {
        Some(format!("{root}/projects/{}", s.to_ascii_uppercase()))
    } else {
        None
    }
}

pub fn target(script: &str, workspace: &str) -> Option<String> {
    script_link(script, workspace).or_else(|| is_link(workspace).then(|| workspace.trim().to_string()))
}

pub fn explain(script: &str, workspace: &str) -> Option<&'static str> {
    let s = script.trim();
    if s.is_empty() || is_link(s) {
        return None;
    }
    if !(is_issue_id(s) || is_project_key(s)) {
        Some("Paste a full link, an issue ID like BL-42, or a project key like BL.")
    } else if workspace_root(workspace).is_none() {
        Some("Set your YouTrack workspace link in Settings → YouTrack to use IDs.")
    } else {
        None
    }
}

pub fn display(url: &str) -> String {
    let bare = url.trim().trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
    if bare.chars().count() > 38 {
        let tail: String = bare.chars().rev().take(36).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        bare.to_string()
    }
}
