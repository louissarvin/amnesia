//! Static judge UI (TanStack Start SPA build) served next to the demo API, under a strict CSP.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

pub struct Site {
    root: PathBuf,
    shell: Vec<u8>,
    csp: HeaderValue,
}

impl Site {
    /// `root` is the build's client directory; the SPA shell is `_shell.html` (TanStack Start).
    pub fn load(root: &Path) -> Result<Arc<Self>> {
        let shell_path = ["_shell.html", "index.html"]
            .iter()
            .map(|n| root.join(n))
            .find(|p| p.is_file())
            .context("web build has no _shell.html or index.html")?;
        let shell = std::fs::read(&shell_path)?;
        let csp = csp_for(&String::from_utf8_lossy(&shell));
        Ok(Arc::new(Self { root: root.canonicalize()?, shell, csp: HeaderValue::from_str(&csp)? }))
    }

    /// Serves `rel`, or its `.gz` twin written at deploy time when the client accepts gzip.
    pub async fn serve(&self, uri: &Uri, gzip: bool) -> Response {
        let Some(rel) = safe_rel(uri.path()) else {
            return (StatusCode::BAD_REQUEST, "bad path").into_response();
        };
        let file = self.root.join(&rel);
        // Stay inside the build even through symlinks.
        let inside = file.canonicalize().is_ok_and(|p| p.starts_with(&self.root) && p.is_file());
        if inside {
            let immutable = rel.starts_with("assets");
            let ct = content_type(&file);
            if gzip && compressible(ct) {
                let gz = file.with_file_name(format!("{}.gz", file.file_name().unwrap_or_default().to_string_lossy()));
                let gz_inside = gz.canonicalize().is_ok_and(|p| p.starts_with(&self.root) && p.is_file());
                if gz_inside && let Ok(bytes) = tokio::fs::read(&gz).await {
                    let mut r = self.respond(bytes, ct, immutable);
                    r.headers_mut().insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
                    r.headers_mut().insert(header::VARY, HeaderValue::from_static("accept-encoding"));
                    return r;
                }
            }
            if let Ok(bytes) = tokio::fs::read(&file).await {
                let mut r = self.respond(bytes, ct, immutable);
                if compressible(ct) {
                    r.headers_mut().insert(header::VARY, HeaderValue::from_static("accept-encoding"));
                }
                return r;
            }
        }
        // Unknown paths are client routes: the shell boots the router.
        self.respond(self.shell.clone(), "text/html; charset=utf-8", false)
    }

    fn respond(&self, bytes: Vec<u8>, ct: &'static str, immutable: bool) -> Response {
        let mut r = Response::new(Body::from(bytes));
        let h = r.headers_mut();
        h.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
        h.insert(header::CONTENT_SECURITY_POLICY, self.csp.clone());
        h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
        h.insert(
            "permissions-policy",
            HeaderValue::from_static("camera=(), microphone=(), geolocation=(), interest-cohort=()"),
        );
        let cc = if immutable { "public, max-age=31536000, immutable" } else { "no-store" };
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cc));
        r
    }
}

/// True when the client lists gzip in Accept-Encoding without `q=0`.
pub fn accepts_gzip(h: &HeaderMap) -> bool {
    h.get_all(header::ACCEPT_ENCODING).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(',')).any(|t| {
        let mut parts = t.split(';').map(str::trim);
        parts.next().is_some_and(|c| c.eq_ignore_ascii_case("gzip")) && !parts.any(|p| p.replace(' ', "") == "q=0")
    })
}

fn compressible(ct: &str) -> bool {
    ct.starts_with("text/") || ct == "application/json" || ct == "image/svg+xml"
}

/// Request path to a relative file path, refusing anything that could leave the build.
fn safe_rel(path: &str) -> Option<PathBuf> {
    let p = Path::new(path.trim_start_matches('/'));
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Scripts only from our origin, plus the exact inline scripts the build emitted (by hash).
/// Inline styles stay allowed: React and Motion set style attributes in the prerendered shell.
fn csp_for(shell: &str) -> String {
    let hashes: Vec<String> = inline_scripts(shell)
        .into_iter()
        .map(|body| format!("'sha256-{}'", b64(&Sha256::digest(body.as_bytes()))))
        .collect();
    format!(
        "default-src 'self'; script-src 'self' {}; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
         font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; \
         frame-ancestors 'none'",
        hashes.join(" ")
    )
}

/// Bodies of `<script>` tags without a `src` attribute.
fn inline_scripts(html: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else { break };
        let tag = &after[..tag_end];
        let body_start = tag_end + 1;
        let Some(close) = after[body_start..].find("</script>") else { break };
        if !tag.contains(" src=") {
            out.push(&after[body_start..body_start + close]);
        }
        rest = &after[body_start + close + "</script>".len()..];
    }
    out
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_negotiation() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::ACCEPT_ENCODING, HeaderValue::from_str(v).unwrap());
            m
        };
        assert!(accepts_gzip(&h("gzip, deflate, br")));
        assert!(accepts_gzip(&h("br;q=1.0, gzip;q=0.8")));
        assert!(!accepts_gzip(&h("gzip;q=0")));
        assert!(!accepts_gzip(&h("br")));
        assert!(!accepts_gzip(&HeaderMap::new()));
    }

    #[test]
    fn refuses_traversal() {
        assert!(safe_rel("/../etc/passwd").is_none());
        assert!(safe_rel("/assets/../../x").is_none());
        assert_eq!(safe_rel("/assets/a.js"), Some(PathBuf::from("assets/a.js")));
        assert_eq!(safe_rel("/"), Some(PathBuf::new()));
    }

    #[test]
    fn hashes_only_inline_scripts() {
        let html = r#"<script src="/a.js"></script><script>alert(1)</script><script type="module">x()</script>"#;
        assert_eq!(inline_scripts(html), vec!["alert(1)", "x()"]);
        // RFC 4648 test vectors.
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"foob"), "Zm9vYg==");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
    }
}
