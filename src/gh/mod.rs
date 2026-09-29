use std::io;
use std::process::{Command, Output, Stdio};

use anyhow::Result;
use serde::Deserialize;

use crate::model::{
    CheckResult, MergeMethod, MergeReadiness, PullRequest, RepoContext, ReviewKind, ReviewState,
};

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
            "number,title,author,baseRefName,headRefName,url,isDraft,reviewDecision,latestReviews",
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

pub fn merge_readiness(repo: &RepoContext, number: u64) -> Result<MergeReadiness> {
    let number_s = number.to_string();
    let output = gh(
        repo,
        &[
            "pr",
            "view",
            &number_s,
            "--json",
            "reviewDecision,mergeable,mergeStateStatus,statusCheckRollup,isDraft",
        ],
    )?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("failed to parse gh pr view: {err}"))?;
    let decision = value
        .get("reviewDecision")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if decision != "APPROVED" {
        return Err(anyhow::anyhow!("pull request is not approved"));
    }
    let repo_out = gh(
        repo,
        &[
            "repo",
            "view",
            "--json",
            "mergeCommitAllowed,squashMergeAllowed,rebaseMergeAllowed",
        ],
    )?;
    let repo_value: serde_json::Value = serde_json::from_slice(&repo_out.stdout)
        .map_err(|err| anyhow::anyhow!("failed to parse gh repo view: {err}"))?;
    let method = MergeMethod::pick(
        flag(&repo_value, "squashMergeAllowed"),
        flag(&repo_value, "mergeCommitAllowed"),
        flag(&repo_value, "rebaseMergeAllowed"),
    );
    let checks: Vec<(String, CheckResult)> = value
        .get("statusCheckRollup")
        .and_then(|item| item.as_array())
        .map(|items| items.iter().map(check_line).collect())
        .unwrap_or_default();
    Ok(MergeReadiness::assess(
        value
            .get("mergeable")
            .and_then(|item| item.as_str())
            .unwrap_or(""),
        value
            .get("mergeStateStatus")
            .and_then(|item| item.as_str())
            .unwrap_or(""),
        value
            .get("isDraft")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        &checks,
        method,
    ))
}

pub fn merge_pr(repo: &RepoContext, number: u64, method: MergeMethod) -> Result<()> {
    let number = number.to_string();
    gh(repo, &["pr", "merge", &number, method.flag()])?;
    Ok(())
}

fn flag(value: &serde_json::Value, name: &str) -> bool {
    value
        .get(name)
        .and_then(|item| item.as_bool())
        .unwrap_or(false)
}

fn check_line(value: &serde_json::Value) -> (String, CheckResult) {
    let name = value
        .get("name")
        .or_else(|| value.get("context"))
        .and_then(|item| item.as_str())
        .unwrap_or("check")
        .to_string();
    (name, check_result(value))
}

fn check_result(value: &serde_json::Value) -> CheckResult {
    let status = value
        .get("status")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let conclusion = value
        .get("conclusion")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let state = value
        .get("state")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if !conclusion.is_empty() {
        return match conclusion {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => CheckResult::Pass,
            _ if status != "COMPLETED" => CheckResult::Pending,
            _ => CheckResult::Fail,
        };
    }
    if !state.is_empty() {
        return match state {
            "SUCCESS" => CheckResult::Pass,
            "PENDING" | "EXPECTED" => CheckResult::Pending,
            _ => CheckResult::Fail,
        };
    }
    if status.is_empty() || status != "COMPLETED" {
        CheckResult::Pending
    } else {
        CheckResult::Pass
    }
}

pub fn review_pr(repo: &RepoContext, number: u64, kind: ReviewKind, body: &str) -> Result<()> {
    let number = number.to_string();
    let mut args = vec!["pr", "review", number.as_str(), kind.flag()];
    if !body.is_empty() {
        args.push("--body-file");
        args.push("-");
    }
    gh_stdin(repo, &args, (!body.is_empty()).then_some(body))?;
    Ok(())
}

fn gh(repo: &RepoContext, args: &[&str]) -> Result<Output> {
    gh_stdin(repo, args, None)
}

fn gh_stdin(repo: &RepoContext, args: &[&str], body: Option<&str>) -> Result<Output> {
    let mut command = Command::new("gh");
    command
        .args(args)
        .current_dir(&repo.root)
        .env("GH_FORCE_TTY", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("GIT_PAGER", "cat")
        .env("GH_EDITOR", ":")
        .env("GIT_EDITOR", ":")
        .env("EDITOR", ":")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if body.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().map_err(spawn_error)?;
    let write_result = if let Some(body) = body {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(body.as_bytes())
        } else {
            Ok(())
        }
    } else {
        Ok(())
    };
    let output = child.wait_with_output().map_err(anyhow::Error::from)?;
    write_result.map_err(|err| anyhow::anyhow!("failed to write review body: {err}"))?;
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
    #[serde(rename = "reviewDecision", default)]
    review_decision: String,
    #[serde(rename = "latestReviews", default)]
    latest_reviews: Vec<GhReview>,
}

#[derive(Deserialize)]
struct GhReview {
    #[serde(default)]
    state: String,
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
            review: ReviewState::from_reviews(
                &item.review_decision,
                item.latest_reviews
                    .iter()
                    .map(|review| review.state.as_str()),
            ),
        }
    }
}
