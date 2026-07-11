//! Reading a markdown file out of an open GitHub pull request.
//!
//! GitHub's diff view is a poor way to read a document: the prose is broken
//! into a table of added and removed lines, wrapped in chrome, and impossible
//! to follow as continuous text. This module takes the URL of a changed file
//! inside a pull request and returns that file's *final* content, the way it
//! will read once merged, so booknook can lay it out as a page instead of a
//! diff.
//!
//! Unlike a gist, a pull request usually lives in a private repository, so
//! this cannot be fetched anonymously over HTTP the way `gist` does. Instead
//! it shells out to the `gh` CLI, which is already authenticated against the
//! user's GitHub account and transparently handles private repos, SSO, and
//! enterprise hosts. The one cost is that `gh` must be installed and logged
//! in; when it is not, the error says so plainly.
//!
//! Like every other source, this module knows nothing about an `App` or a
//! terminal. It turns a URL into a `(title, markdown)` pair and hands it back.

use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};

/// Whether `arg` is a link to a file inside a GitHub pull request, as opposed
/// to a gist, a bare path, or an ordinary web URL. A PR link has `/pull/<n>`
/// in it; that is enough to route it here rather than to the gist fetcher.
pub(crate) fn looks_like_pr(arg: &str) -> bool {
    (arg.starts_with("http://") || arg.starts_with("https://"))
        && arg.contains("github.com/")
        && arg.contains("/pull/")
}

/// The pieces pulled out of a pull-request URL: which repository, which pull
/// request, and, if the link points at one specific changed file, the anchor
/// that identifies it. GitHub writes that anchor as `diff-<sha>`, where the
/// sha is the SHA-256 of the file's path, which is what lets [`fetch`] turn it
/// back into a filename.
struct PrRef {
    owner: String,
    repo: String,
    number: String,
    /// The `sha256(filename)` taken from a `#diff-<sha>` fragment, if the URL
    /// carried one. `None` for a link to the pull request as a whole, in which
    /// case the first changed markdown file is opened instead.
    file_anchor: Option<String>,
}

/// Fetch the markdown of the file a pull-request URL points at, as it stands
/// at the tip of the PR branch.
///
/// The steps are: understand the URL, ask `gh` for the PR's list of changed
/// files and its head commit, decide which of those files the URL means, then
/// read that file's contents at the head commit. Each step surfaces a plain
/// error rather than a panic, so a private repo the user cannot see, or a PR
/// with no markdown in it, ends in a message instead of a crash.
pub(crate) fn fetch(url: &str) -> Result<(String, String)> {
    let pr = parse_url(url)?;
    let slug = format!("{}/{}", pr.owner, pr.repo);

    let filename = resolve_file(&pr, &slug)?;
    let head_sha = pr_head_sha(&slug, &pr.number)?;
    let content = file_at_ref(&slug, &filename, &head_sha)?;

    // The title is the file's own name plus the PR number, so the header reads
    // like "readme.md (repo#42)" and tells the two apart when several PRs are
    // opened in a session.
    let leaf = filename.rsplit('/').next().unwrap_or(&filename);
    let title = format!("{leaf} ({}#{})", pr.repo, pr.number);
    Ok((title, content))
}

/// Break a pull-request URL into its parts. Accepts the shapes GitHub hands
/// out: the PR overview, its `/files` tab, and a link deep-linked to one
/// file's diff via a `#diff-<sha>` fragment.
fn parse_url(url: &str) -> Result<PrRef> {
    // Separate the fragment (`#diff-...`) from the path before splitting, so
    // the anchor does not get caught up in the path segments.
    let (path_part, fragment) = match url.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (url, None),
    };

    // Everything after `github.com/` is `owner/repo/pull/<n>/...`. Splitting on
    // `/` and reading positionally is enough; the host and scheme ahead of it
    // are ignored, so both github.com and an enterprise host work.
    let after_host = path_part
        .split("github.com/")
        .nth(1)
        .ok_or_else(|| anyhow!("{url} is not a github.com URL"))?;
    let mut segments = after_host.split('/').filter(|s| !s.is_empty());

    let owner = segments.next().ok_or_else(|| anyhow!("no owner in {url}"))?;
    let repo = segments.next().ok_or_else(|| anyhow!("no repo in {url}"))?;
    match segments.next() {
        Some("pull") => {}
        _ => bail!("{url} is not a pull-request URL"),
    }
    let number = segments.next().ok_or_else(|| anyhow!("no PR number in {url}"))?;

    // A `diff-<sha>` fragment names one file; anything else (like a comment
    // anchor) is not a file selector and is treated as no selection.
    let file_anchor = fragment
        .and_then(|f| f.strip_prefix("diff-"))
        .map(str::to_string);

    Ok(PrRef {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number: number.to_string(),
        file_anchor,
    })
}

/// Decide which changed file in the PR the URL refers to. A `#diff-<sha>`
/// anchor is matched against the SHA-256 of each changed file's path, the
/// exact scheme GitHub uses to build those anchors. With no anchor, the first
/// changed markdown file is chosen, which is the common case of a PR that adds
/// or edits a single document.
fn resolve_file(pr: &PrRef, slug: &str) -> Result<String> {
    let files = changed_files(slug, &pr.number)?;
    if files.is_empty() {
        bail!("pull request {slug}#{} changes no files", pr.number);
    }

    if let Some(anchor) = &pr.file_anchor {
        return files
            .iter()
            .find(|name| sha256_hex(name) == *anchor)
            .cloned()
            .ok_or_else(|| {
                anyhow!("no file in {slug}#{} matches the link's anchor", pr.number)
            });
    }

    files
        .iter()
        .find(|name| is_markdown_name(name))
        .cloned()
        .ok_or_else(|| anyhow!("pull request {slug}#{} changes no markdown files", pr.number))
}

/// The paths of every file the PR touches, via `gh api .../pulls/<n>/files`.
/// Paginated so a large PR is listed in full rather than truncated at the
/// first page.
fn changed_files(slug: &str, number: &str) -> Result<Vec<String>> {
    let out = gh(&[
        "api",
        &format!("repos/{slug}/pulls/{number}/files"),
        "--paginate",
        "--jq",
        ".[].filename",
    ])?;
    Ok(out.lines().map(str::to_string).collect())
}

/// The SHA of the PR branch's head commit, the ref at which files are read so
/// that what booknook shows is the file as the PR would merge it.
fn pr_head_sha(slug: &str, number: &str) -> Result<String> {
    let out = gh(&[
        "api",
        &format!("repos/{slug}/pulls/{number}"),
        "--jq",
        ".head.sha",
    ])?;
    let sha = out.trim();
    if sha.is_empty() {
        bail!("could not read the head commit of {slug}#{number}");
    }
    Ok(sha.to_string())
}

/// Read one file's contents at a given commit. The contents API returns the
/// body base64-encoded; `gh` is asked to decode it with a `.jq` expression so
/// no base64 crate is needed here.
fn file_at_ref(slug: &str, path: &str, sha: &str) -> Result<String> {
    gh(&[
        "api",
        &format!("repos/{slug}/contents/{path}?ref={sha}"),
        "--jq",
        ".content | @base64d",
    ])
}

/// Run `gh` with the given arguments and return its stdout. A missing binary
/// and a non-zero exit are told apart, because the first means "install gh"
/// and the second usually means the repo is private to someone else or the
/// login has lapsed, and the fixes differ.
fn gh(args: &[&str]) -> Result<String> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .context("could not run `gh`; is the GitHub CLI installed and on your PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`gh` failed: {}", stderr.trim());
    }

    String::from_utf8(output.stdout).context("`gh` returned output that was not valid UTF-8")
}

/// The lowercase hex SHA-256 of a string, matching how GitHub builds the
/// `diff-<sha>` anchors that deep-link to a file in a PR.
fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether a path names a markdown file, judged by extension alone. Mirrors
/// `browser::is_markdown` but works on a string path from the API rather than
/// a `Path` on disk.
fn is_markdown_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_pr_file_link() {
        assert!(looks_like_pr("https://github.com/org/repo/pull/291/files"));
        assert!(looks_like_pr("https://github.com/org/repo/pull/7"));
        assert!(!looks_like_pr("https://github.com/org/repo/issues/7"));
        assert!(!looks_like_pr("https://gist.github.com/user/abc"));
        assert!(!looks_like_pr("notes.md"));
    }

    #[test]
    fn parses_owner_repo_number_and_anchor() {
        let pr =
            parse_url("https://github.com/octocat/hello/pull/42/changes#diff-abc123").unwrap();
        assert_eq!(pr.owner, "octocat");
        assert_eq!(pr.repo, "hello");
        assert_eq!(pr.number, "42");
        assert_eq!(pr.file_anchor.as_deref(), Some("abc123"));
    }

    #[test]
    fn parses_a_bare_pr_link_without_an_anchor() {
        let pr = parse_url("https://github.com/org/repo/pull/12").unwrap();
        assert_eq!(pr.number, "12");
        assert!(pr.file_anchor.is_none());
    }

    #[test]
    fn a_non_pr_url_is_rejected() {
        assert!(parse_url("https://github.com/org/repo/issues/12").is_err());
    }

    #[test]
    fn the_anchor_is_the_sha256_of_the_path() {
        // GitHub builds a file's `diff-<sha>` anchor as the SHA-256 of its
        // path. Pinning a known digest here proves the hash booknook resolves
        // against is that one. `sha256sum <<< 'README.md'` (with no trailing
        // newline) reproduces this value.
        assert_eq!(
            sha256_hex("README.md"),
            "b335630551682c19a781afebcf4d07bf978fb1f8ac04c6bf87428ed5106870f5"
        );
    }
}
