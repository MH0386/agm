//! Registry clients that fetch skills from remote sources.

pub mod archive;
pub mod github;

use agm_core::registry::{Registry, RegistrySource};

pub use github::GitHubClient;

/// Turns a [`RegistrySource`] into a client that can fetch skills.
pub fn registry_for(source: RegistrySource) -> impl Registry {
    match source {
        RegistrySource::GitHub { owner, repo } => GitHubClient { owner, repo },
    }
}
