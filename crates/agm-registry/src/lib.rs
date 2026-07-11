pub mod github;

use agm_core::registry::{Registry, RegistrySource};

pub use github::GitHubRegistry;

pub fn registry_for(source: RegistrySource) -> impl Registry {
    match source {
        RegistrySource::GitHub { owner, repo } => GitHubRegistry { owner, repo },
    }
}
