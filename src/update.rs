#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Off,
    Checking,
    Latest,
    Newer(Release),
    Failed,
}

pub const RELEASES_API: &str = "https://api.github.com/repos/NativeCompute/blackline/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/NativeCompute/blackline/releases";

fn parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

pub fn newer(current: &str, latest: &str) -> bool {
    let (mut a, mut b) = (parts(current), parts(latest));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    b > a
}

pub fn parse(json: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = v.get("tag_name")?.as_str()?;
    let url = v
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or(RELEASES_PAGE);
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        url: url.to_string(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn() -> std::sync::mpsc::Receiver<Status> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(check());
    });
    rx
}

#[cfg(not(target_arch = "wasm32"))]
fn check() -> Status {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(concat!("Blackline/", env!("CARGO_PKG_VERSION")))
        .build();
    let body = match agent
        .get(RELEASES_API)
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r.into_string().ok(),
        Err(ureq::Error::Status(404, _)) => return Status::Latest,
        Err(_) => None,
    };
    match body.as_deref().and_then(parse) {
        Some(r) if newer(env!("CARGO_PKG_VERSION"), &r.version) => Status::Newer(r),
        Some(_) => Status::Latest,
        None => Status::Failed,
    }
}
