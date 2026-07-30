//! Registry clients that fetch skills from remote sources.

pub mod archive;
pub mod github;

use agm_core::registry::{Registry, RegistrySource};

pub use github::GitHubRegistry;

/// Turns a [`RegistrySource`] into a registry that can fetch skills.
pub fn registry_for(source: RegistrySource) -> impl Registry {
    match source {
        RegistrySource::GitHub { owner, repo_name } => GitHubRegistry { owner, repo_name },
    }
}
