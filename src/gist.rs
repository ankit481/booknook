//! Reading a GitHub gist as if it were a local file.
//!
//! booknook normally opens a path on disk. A gist is the one case where the
//! text lives on the network instead. This module is the whole of that
//! exception: it recognizes a gist URL, turns it into the address of the
//! gist's raw markdown, and fetches that text. Everything downstream, the
//! parser, the wrapper, the reader, sees only a `String` and never learns
//! where it came from.
//!
//! It has no idea that an `App` or a terminal exists. `main` decides, from
//! the command-line argument, whether to reach for a file or for this, and
//! feeds the result into the reader the same way either path would.

use anyhow::{Context, Result, bail};

/// Whether `arg` looks like a URL we should fetch rather than a path we
/// should open. Kept deliberately loose: anything with an `http(s)` scheme is
/// treated as remote, so a raw gist link, a `gist.github.com` link, or any
/// other markdown URL all take the network path. A bare filename never does.
pub(crate) fn looks_remote(arg: &str) -> bool {
    arg.starts_with("http://") || arg.starts_with("https://")
}

/// Fetch a gist (or any remote markdown URL) and return its text.
///
/// The address is first normalized to a raw endpoint by [`raw_url`], so the
/// caller can pass the link straight from a browser's address bar. The
/// response body is returned verbatim, ready to hand to the markdown parser
/// exactly as a file's contents would be.
pub(crate) fn fetch(url: &str) -> Result<String> {
    let target = raw_url(url);
    let response = ureq::get(&target)
        .call()
        .with_context(|| format!("could not fetch {target}"))?;

    // A gist that does not exist, or a link mistyped, comes back as an error
    // status rather than markdown. Surfacing it here means the reader never
    // opens on a page of HTML error text.
    if response.status() != 200 {
        bail!("{target} returned HTTP {}", response.status());
    }

    response
        .into_string()
        .with_context(|| format!("could not read the body of {target}"))
}

/// A short, human-facing name for a fetched gist, shown where a file's path
/// would otherwise appear in the reader's header. The gist id is enough to
/// tell one open gist from another without spelling out the whole URL.
pub(crate) fn title(url: &str) -> String {
    match gist_id(url) {
        Some(id) => format!("gist:{id}"),
        None => url.to_string(),
    }
}

/// Turn a gist URL into the address of its raw markdown.
///
/// `gist.github.com/user/<id>` serves the rendered gist page, which is HTML,
/// not the markdown behind it. Appending `/raw` asks GitHub for the gist's
/// raw text instead: for a single-file gist that is the file, and for a
/// multi-file gist it is every file concatenated, which reads as one
/// document. A link that is already a raw endpoint, on the
/// `gist.githubusercontent.com` host or ending in `/raw`, is left untouched,
/// as is any non-gist URL, so a direct link to a raw `.md` file elsewhere
/// still works.
fn raw_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');

    // Already raw: the usercontent host always serves raw bytes, and an
    // explicit `/raw` suffix is a raw endpoint by definition.
    if trimmed.contains("gist.githubusercontent.com") || trimmed.ends_with("/raw") {
        return trimmed.to_string();
    }

    // A browser link often carries a `#file-...` fragment naming which file
    // is in view. The `/raw` endpoint returns the whole gist regardless, so
    // the fragment is dropped rather than honored: simpler, and it still
    // shows the file the reader asked for, alongside the rest.
    let base = trimmed.split('#').next().unwrap_or(trimmed);

    if base.contains("gist.github.com") {
        format!("{}/raw", base.trim_end_matches('/'))
    } else {
        // Some other URL entirely. Assume it already points at raw text, such
        // as a link to a raw file on a plain web server, and fetch it as-is.
        base.to_string()
    }
}

/// The gist's id, the last path segment of a `gist.github.com` link, used
/// only to label the open document. `None` for any URL that is not a
/// recognizable gist link, in which case the caller falls back to the URL
/// itself.
fn gist_id(url: &str) -> Option<String> {
    let base = url.trim_end_matches('/').split('#').next()?;
    if !base.contains("gist.github.com") {
        return None;
    }
    base.rsplit('/')
        .find(|segment| !segment.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_gist_link_gets_a_raw_suffix() {
        assert_eq!(
            raw_url("https://gist.github.com/octocat/aa5a315d61ae9438b18d"),
            "https://gist.github.com/octocat/aa5a315d61ae9438b18d/raw"
        );
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        assert_eq!(
            raw_url("https://gist.github.com/octocat/aa5a315d61ae9438b18d/"),
            "https://gist.github.com/octocat/aa5a315d61ae9438b18d/raw"
        );
    }

    #[test]
    fn a_file_fragment_is_dropped() {
        assert_eq!(
            raw_url("https://gist.github.com/octocat/aa5a315d61ae9438b18d#file-notes-md"),
            "https://gist.github.com/octocat/aa5a315d61ae9438b18d/raw"
        );
    }

    #[test]
    fn an_already_raw_link_is_untouched() {
        let raw = "https://gist.githubusercontent.com/octocat/aa5a/raw/abc/notes.md";
        assert_eq!(raw_url(raw), raw);
    }

    #[test]
    fn an_explicit_raw_suffix_is_untouched() {
        let raw = "https://gist.github.com/octocat/aa5a315d61ae9438b18d/raw";
        assert_eq!(raw_url(raw), raw);
    }

    #[test]
    fn the_id_labels_the_document() {
        assert_eq!(
            title("https://gist.github.com/octocat/aa5a315d61ae9438b18d"),
            "gist:aa5a315d61ae9438b18d"
        );
    }

    #[test]
    fn only_http_urls_are_remote() {
        assert!(looks_remote("https://gist.github.com/x/y"));
        assert!(looks_remote("http://example.com/notes.md"));
        assert!(!looks_remote("notes.md"));
        assert!(!looks_remote("/home/user/notes.md"));
    }
}
