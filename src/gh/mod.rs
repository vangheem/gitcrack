use std::io;
use std::process::{Command, Output, Stdio};

use anyhow::Result;
use serde::Deserialize;

use crate::model::{PullRequest, RepoContext};

pub fn list_prs(repo: &RepoContext) -> Result<Vec<PullRequest>> {
    let output = gh(
        repo,
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "100",
            "--json",
            "number,title,author,baseRefName,headRefName,url,isDraft",
        ],
    )?;
    let items: Vec<GhPullRequest> = serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("failed to parse gh pr list: {err}"))?;
    Ok(items.into_iter().map(PullRequest::from).collect())
}

pub fn pr_diff(repo: &RepoContext, number: u64) -> Result<String> {
    let number = number.to_string();
    let output = gh(repo, &["pr", "diff", &number])?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn gh(repo: &RepoContext, args: &[&str]) -> Result<Output> {
    let output = Command::new("gh")
        .args(args)
        .current_dir(&repo.root)
        .env("GH_FORCE_TTY", "0")
        .env("NO_COLOR", "1")
        .env("GIT_PAGER", "cat")
        .stdin(Stdio::null())
        .output()
        .map_err(spawn_error)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(exit_error(&output))
    }
}

fn spawn_error(err: io::Error) -> anyhow::Error {
    if err.kind() == io::ErrorKind::NotFound {
        anyhow::anyhow!("gh cli not found")
    } else {
        anyhow::Error::from(err)
    }
}

fn exit_error(output: &Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    anyhow::anyhow!("gh exited with {}: {}", output.status, stderr.trim())
}

#[derive(Deserialize)]
struct GhPullRequest {
    number: u64,
    title: String,
    #[serde(default)]
    author: Option<GhAuthor>,
    #[serde(rename = "baseRefName")]
    base_ref: String,
    #[serde(rename = "headRefName")]
    head_ref: String,
    url: String,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
}

#[derive(Deserialize)]
struct GhAuthor {
    login: String,
}

impl From<GhPullRequest> for PullRequest {
    fn from(item: GhPullRequest) -> Self {
        Self {
            number: item.number,
            title: item.title,
            author: item
                .author
                .map(|author| author.login)
                .unwrap_or_else(|| "unknown".to_string()),
            base_ref: item.base_ref,
            head_ref: item.head_ref,
            url: item.url,
            is_draft: item.is_draft,
        }
    }
}
