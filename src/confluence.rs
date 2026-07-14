//! Reading a Confluence Cloud page as a booknook document.
//!
//! A Confluence page is where a lot of real reading lives, design docs,
//! TDDs, runbooks, and the browser view wraps it in chrome the same way
//! GitHub's diff view does to a PR. This module takes the URL of a page,
//! fetches its rendered body over the Confluence REST API, and converts
//! that HTML to markdown, so everything downstream, the parser, the
//! wrapper, the reader, sees the same `(title, markdown)` pair `pr` hands
//! back and never learns where it came from.
//!
//! The API asks for `export_view`, the fully rendered body, rather than
//! `storage`, the raw editor format. Rendered HTML has Confluence's macros,
//! info panels, expands, Jira links, already flattened into ordinary
//! markup, which is exactly what a converter can digest; storage format is
//! studded with `<ac:structured-macro>` elements that would each need
//! interpreting by hand.
//!
//! Unlike a gist, a Confluence page lives behind a login. There is no CLI
//! to borrow a session from the way `pr` borrows `gh`'s, so this module
//! borrows the next simplest thing: an API token from the environment.
//! `CONFLUENCE_EMAIL` and `CONFLUENCE_API_TOKEN` are read at fetch time,
//! and when they are missing the error says where to create a token and
//! what to export. Set once in a shell profile, they never come up again.

use anyhow::{Context, Result, anyhow, bail};

/// Whether `arg` is a link to a Confluence Cloud page, as opposed to a gist,
/// a PR, or a bare path. A page link lives under `<site>.atlassian.net/wiki/`
/// and has `/pages/` in its path; that is enough to route it here rather
/// than to the anonymous fetch a gist uses.
pub(crate) fn looks_like_confluence(arg: &str) -> bool {
    (arg.starts_with("http://") || arg.starts_with("https://"))
        && arg.contains(".atlassian.net/wiki/")
        && arg.contains("/pages/")
}

/// Fetch the page a Confluence URL points at and return its title and its
/// body converted to markdown.
///
/// The steps are: pull the site and the page id out of the URL, read the
/// token from the environment, ask the v2 REST API for the page with its
/// rendered body, then convert that HTML to markdown. Each step surfaces a
/// plain error rather than a panic, so a missing token, a lapsed one, or a
/// page the account cannot see ends in a message instead of a crash.
pub(crate) fn fetch(url: &str) -> Result<(String, String)> {
    let site = site_of(url)?;
    let id = page_id(url)?;
    let auth = credentials()?;

    let target = format!("{site}/wiki/api/v2/pages/{id}?body-format=export_view");
    let response = match ureq::get(&target).set("Authorization", &auth).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(401, _)) => {
            bail!("Confluence rejected the token; check CONFLUENCE_EMAIL and CONFLUENCE_API_TOKEN")
        }
        // Confluence answers 404 for a page that does not exist, for one the
        // account cannot see, and, unhelpfully, for a request whose token
        // never authenticated at all. A follow-up call tells those apart, so
        // the message can point at the actual problem.
        Err(ureq::Error::Status(404, _)) => {
            if authenticates(&site, &auth) {
                bail!("page {id} was not found on {site}, or this account cannot see it")
            }
            bail!(
                "Confluence did not accept the token, which it reports as a 404. \
                 A token created \"with scopes\" does not work here: create a classic one \
                 via plain \"Create API token\" at \
                 https://id.atlassian.com/manage-profile/security/api-tokens"
            )
        }
        Err(ureq::Error::Status(code, _)) => bail!("{target} returned HTTP {code}"),
        Err(err) => return Err(err).with_context(|| format!("could not fetch {target}")),
    };

    let body = response
        .into_string()
        .with_context(|| format!("could not read the body of {target}"))?;
    let page: serde_json::Value =
        serde_json::from_str(&body).context("Confluence returned a response that was not JSON")?;

    let title = page["title"].as_str().unwrap_or("confluence page").to_string();
    let html = page["body"]["export_view"]["value"]
        .as_str()
        .ok_or_else(|| anyhow!("the response for page {id} carried no rendered body"))?;

    let markdown = htmd::convert(html)
        .map_err(|err| anyhow!("could not convert the page's HTML to markdown: {err}"))?;
    Ok((title, markdown))
}

/// Whether the credentials actually authenticate against the site, checked
/// only on the failure path. `user/current` answers 200 for a logged-in
/// account and 403 for an anonymous or badly-authenticated request, which is
/// the distinction the page fetch's 404 refuses to make.
fn authenticates(site: &str, auth: &str) -> bool {
    ureq::get(&format!("{site}/wiki/rest/api/user/current"))
        .set("Authorization", auth)
        .call()
        .is_ok()
}

/// The site half of the URL, scheme and host, everything ahead of `/wiki/`.
/// Kept as a whole prefix rather than reparsed, so the API call goes back to
/// exactly the host the link named.
fn site_of(url: &str) -> Result<String> {
    url.split("/wiki/")
        .next()
        .filter(|site| site.contains(".atlassian.net"))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{url} is not a Confluence Cloud URL"))
}

/// The page id, the run of digits that follows `pages` in the path. Reading
/// it positionally but skipping non-numeric segments covers the shapes
/// Confluence hands out: `/pages/<id>/<title>` from the address bar and
/// `/pages/edit-v2/<id>` from the editor. The title slug after the id is
/// ignored; the id alone names the page.
fn page_id(url: &str) -> Result<String> {
    let path = url.split('#').next().unwrap_or(url);
    let mut segments = path.split('/');
    // Advance to the `pages` segment, then take the first purely numeric
    // segment after it.
    for segment in segments.by_ref() {
        if segment == "pages" {
            break;
        }
    }
    segments
        .find(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no page id in {url}"))
}

/// The `Authorization` header value, built from the two environment
/// variables. Confluence Cloud API tokens authenticate as HTTP basic auth
/// with the account's email as the username, so the pair is joined and
/// base64-encoded the way basic auth requires.
fn credentials() -> Result<String> {
    let email = std::env::var("CONFLUENCE_EMAIL");
    let token = std::env::var("CONFLUENCE_API_TOKEN");
    match (email, token) {
        (Ok(email), Ok(token)) => Ok(format!("Basic {}", base64(&format!("{email}:{token}")))),
        _ => bail!(
            "reading a Confluence page needs CONFLUENCE_EMAIL and CONFLUENCE_API_TOKEN set; \
             create a token at https://id.atlassian.com/manage-profile/security/api-tokens"
        ),
    }
}

/// Standard base64, the flavor HTTP basic auth expects. Encoding is the only
/// direction ever needed and it is a dozen lines, so it is written out here
/// rather than pulling in a crate for it, the same call `pr` made when it
/// left decoding to `gh`.
fn base64(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.as_bytes().chunks(3) {
        let n = u32::from_be_bytes([
            0,
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ]);
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_confluence_page_link() {
        assert!(looks_like_confluence(
            "https://acme.atlassian.net/wiki/spaces/ENG/pages/123456/Some+Design+Doc"
        ));
        assert!(!looks_like_confluence("https://acme.atlassian.net/browse/ENG-42"));
        assert!(!looks_like_confluence("https://gist.github.com/user/abc"));
        assert!(!looks_like_confluence("notes.md"));
    }

    #[test]
    fn the_site_is_everything_ahead_of_wiki() {
        assert_eq!(
            site_of("https://acme.atlassian.net/wiki/spaces/ENG/pages/123456/Doc").unwrap(),
            "https://acme.atlassian.net"
        );
        assert!(site_of("https://example.com/wiki/pages/1").is_err());
    }

    #[test]
    fn the_page_id_is_the_digits_after_pages() {
        let url = "https://acme.atlassian.net/wiki/spaces/ENG/pages/4374822916/TDD+Follow+Up";
        assert_eq!(page_id(url).unwrap(), "4374822916");
    }

    #[test]
    fn an_editor_link_still_yields_the_id() {
        let url = "https://acme.atlassian.net/wiki/spaces/ENG/pages/edit-v2/4374822916";
        assert_eq!(page_id(url).unwrap(), "4374822916");
    }

    #[test]
    fn a_link_without_an_id_is_rejected() {
        assert!(page_id("https://acme.atlassian.net/wiki/spaces/ENG/pages/").is_err());
    }

    #[test]
    fn basic_auth_base64_matches_the_reference_encoding() {
        // `printf 'user@example.com:token123' | base64` reproduces this value,
        // covering all three padding cases via the input's length.
        assert_eq!(base64("user@example.com:token123"), "dXNlckBleGFtcGxlLmNvbTp0b2tlbjEyMw==");
        assert_eq!(base64("a"), "YQ==");
        assert_eq!(base64("ab"), "YWI=");
        assert_eq!(base64("abc"), "YWJj");
    }

    #[test]
    fn tables_survive_the_html_conversion() {
        // Confluence TDDs lean on tables, so the html-to-markdown step must
        // produce pipe tables the markdown parser can lay out as a grid, not
        // flattened cell text.
        let html = "<table><tr><th>Name</th><th>Value</th></tr>\
                    <tr><td>alpha</td><td>1</td></tr></table>";
        let markdown = htmd::convert(html).unwrap();
        assert!(markdown.contains('|'), "no pipe table in: {markdown}");
    }
}
