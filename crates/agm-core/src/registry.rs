use color_eyre::eyre::{Result, bail};

/// Represents a parsed registry source identifier.
#[derive(Debug, Clone)]
pub enum RegistrySource {
    GitHub { owner: String, repo: String },
}

/// Parses a source string like `github:owner/repo` or `https://github.com/owner/repo` into a `RegistrySource`.
pub fn parse_source(input: &str) -> Result<RegistrySource> {
    if input.is_empty() {
        bail!("Source cannot be empty")
    }

    let rest = if let Some(rest) = input.strip_prefix("github:") {
        rest.trim_end_matches('/')
    } else if let Some(rest) = input.strip_prefix("https://github.com/") {
        rest.trim_end_matches('/')
    } else {
        bail!(
            "Invalid source identifier: expected `github:owner/repo` or `https://github.com/owner/repo`, got `{}`",
            input
        );
    };

    let Some((owner, repo)) = rest
        .split_once('/')
        .filter(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/'))
    else {
        bail!(
            "Invalid source github repository format: expected `github:owner/repo` or `https://github.com/owner/repo`, got `{}`",
            input
        );
    };

    Ok(RegistrySource::GitHub {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}
