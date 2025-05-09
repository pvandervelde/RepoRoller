//! Crate for interacting with the GitHub REST API.
//!
//! This crate provides a client for making authenticated requests to GitHub,
//! authenticating as a GitHub App using its ID and private key.

use jsonwebtoken::EncodingKey;
use octocrab::models::Repository;
use octocrab::{Error as OctocrabError, Octocrab, Result as OctocrabResult}; // Added OctocrabResult
use serde::Serialize;
use thiserror::Error;
use tracing::instrument;

// Reference the tests module in the separate file
#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

/// Custom error type for the `github_client`.
#[derive(Error, Debug)]
pub enum Error {
    /// Error originating from the underlying `octocrab` client.
    #[error("GitHub API error: {0}")]
    Octocrab(#[from] OctocrabError),

    /// Error during client authentication or initialization.
    #[error("Failed to authenticate or initialize GitHub client: {0}")]
    AuthError(String),

    /// Error deserializing the response from GitHub.
    #[error("Failed to deserialize GitHub response: {0}")]
    Deserialization(#[from] serde_json::Error),
}

/// Represents the settings that can be updated for a repository.
/// Use `Default::default()` and modify fields as needed.
#[derive(Serialize, Default, Debug)]
pub struct RepositorySettingsUpdate<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_issues: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_projects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_wiki: Option<bool>,
    // Add other updatable fields like topics, default_branch etc. as needed
}

/// Represents the payload for creating a new repository via the REST API.
/// Use `Default::default()` or builder pattern and modify fields as needed.
#[derive(Serialize, Default, Debug, Clone)] // Added Clone
pub struct RepositoryCreatePayload<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>, // Defaults to false if None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_issues: Option<bool>, // Defaults to true if None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_projects: Option<bool>, // Defaults to true if None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_wiki: Option<bool>, // Defaults to true if None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_template: Option<bool>, // Defaults to false if None
                                   // Add other creation fields like team_id, auto_init, gitignore_template etc. as needed
}

/// A client for interacting with the GitHub API, authenticated as a GitHub App.
#[derive(Debug)]
pub struct GitHubClient {
    client: Octocrab,
}

impl GitHubClient {
    /// Creates a file in the default branch of a repository using the GitHub REST API.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository (user or organization name).
    /// * `repo` - The name of the repository.
    /// * `path` - The path to the file in the repository.
    /// * `content` - The raw file content (will be base64 encoded).
    /// * `commit_message` - The commit message to use.
    ///
    /// # Errors
    /// Returns an `Error::Octocrab` if the API call fails.
    #[instrument(skip(self, content, commit_message), fields(owner = %owner, repo = %repo, path = %path))]
    pub async fn create_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &[u8],
        commit_message: &str,
    ) -> Result<(), Error> {
        use base64::{engine::general_purpose, Engine as _};
        use serde_json::json;

        let encoded_content = general_purpose::STANDARD.encode(content);

        let body = json!({
            "message": commit_message,
            "content": encoded_content,
        });

        let api_path = format!("/repos/{}/{}/contents/{}", owner, repo, path);
        let response = self.client._put(api_path, Some(&body)).await;

        match response {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::Octocrab(e)),
        }
    }

    /// Creates a new repository within a specified organization using the REST API directly.
    ///
    /// # Arguments
    ///
    /// * `org_name` - The name of the organization.
    /// * `payload` - A `RepositoryCreatePayload` struct containing the repository details.
    ///
    /// # Errors
    /// Returns `Error::Octocrab` for API errors or `Error::Deserialization` if the response cannot be parsed.
    #[instrument(skip(self, payload), fields(org = %org_name, repo_name = %payload.name))]
    pub async fn create_org_repository(
        &self,
        org_name: &str,
        payload: &RepositoryCreatePayload<'_>,
    ) -> Result<Repository, Error> {
        let path = format!("/orgs/{}/repos", org_name);
        let response: OctocrabResult<Repository> = self.client.post(path, Some(payload)).await;
        response.map_err(Error::Octocrab)
    }

    /// Creates a new repository for the authenticated user (GitHub App) using the REST API directly.
    ///
    /// # Arguments
    ///
    /// * `payload` - A `RepositoryCreatePayload` struct containing the repository details.
    ///
    /// # Errors
    /// Returns `Error::Octocrab` for API errors or `Error::Deserialization` if the response cannot be parsed.
    #[instrument(skip(self, payload), fields(repo_name = %payload.name))]
    pub async fn create_user_repository(
        &self,
        payload: &RepositoryCreatePayload<'_>,
    ) -> Result<Repository, Error> {
        let path = "/user/repos";
        let response: OctocrabResult<Repository> = self.client.post(path, Some(payload)).await;
        response.map_err(Error::Octocrab)
    }

    /// Fetches details for a specific repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository (user or organization name).
    /// * `repo` - The name of the repository.
    ///
    /// # Errors
    /// Returns an `Error::Octocrab` if the API call fails.
    #[instrument(skip(self), fields(owner = %owner, repo = %repo))]
    pub async fn get_repository(&self, owner: &str, repo: &str) -> Result<Repository, Error> {
        self.client
            .repos(owner, repo)
            .get()
            .await
            .map_err(Error::Octocrab)
    }

    /// Creates a new `GitHubClient` instance authenticated as a GitHub App.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The ID of the GitHub App.
    /// * `private_key` - The private key associated with the GitHub App, in PEM format.
    ///
    /// # Errors
    /// Returns an `Error::AuthError` if authentication or client building fails.
    #[instrument(skip(private_key), fields(app_id = %app_id))]
    pub async fn new(app_id: u64, private_key: String) -> Result<Self, Error> {
        let key = EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|e| {
            Error::AuthError(format!("Failed to parse GitHub App private key: {}", e))
        })?;

        let octocrab = Octocrab::builder()
            .app(app_id.into(), key)
            .build()
            .map_err(|e| {
                Error::AuthError(format!(
                    "Failed to build Octocrab client for GitHub App: {}",
                    e
                ))
            })?;

        Ok(Self { client: octocrab })
    }

    /// Updates settings for a specific repository using the REST API directly.
    ///
    /// Only the fields provided in the `settings` argument will be updated.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository (user or organization name).
    /// * `repo` - The name of the repository.
    /// * `settings` - A `RepositorySettingsUpdate` struct containing the desired changes.
    ///
    /// # Errors
    /// Returns an `Error::Octocrab` if the API call fails.
    #[instrument(skip(self, settings), fields(owner = %owner, repo = %repo))]
    pub async fn update_repository_settings(
        &self,
        owner: &str,
        repo: &str,
        settings: &RepositorySettingsUpdate<'_>,
    ) -> Result<Repository, Error> {
        let path = format!("/repos/{}/{}", owner, repo);
        // Use client.patch for updating repository settings via the REST API
        let response: OctocrabResult<Repository> = self.client.patch(path, Some(settings)).await;
        response.map_err(Error::Octocrab)
    }
}
