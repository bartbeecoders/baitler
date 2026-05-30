//! Shared document conversion/export pathway.
//!
//! Everything funnels through sanitized HTML:
//! - Markdown ↔ HTML (`pulldown-cmark` / `html2md`)
//! - HTML sanitization (`ammonia`) — applied before storing and before
//!   rendering, which removes scripts/handlers (XSS) and shrinks the
//!   server-side-render SSRF surface.
//! - HTML → PDF via headless Chrome (`CHROME_BIN`, default `google-chrome-stable`)
//! - HTML → DOCX via `pandoc` (`PANDOC_BIN`) when installed, else a clear error.
//!
//! Excel/PowerPoint export is intentionally out of scope here (those target
//! structured data, not prose documents).

use std::time::Duration;

use pulldown_cmark::{html, Options, Parser};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("unsupported format")]
    UnsupportedFormat,
    #[error("{0} export is not available on this server")]
    NotConfigured(&'static str),
    #[error("conversion failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    Html,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFormat {
    Html,
    Markdown,
    Pdf,
    Docx,
}

impl SourceFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "html" => Some(Self::Html),
            "markdown" | "md" => Some(Self::Markdown),
            _ => None,
        }
    }
}

impl TargetFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "html" => Some(Self::Html),
            "markdown" | "md" => Some(Self::Markdown),
            "pdf" => Some(Self::Pdf),
            "docx" | "word" => Some(Self::Docx),
            _ => None,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "md",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }
}

/// Markdown → HTML (GitHub-flavored).
pub fn md_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(markdown, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// HTML → Markdown.
pub fn html_to_md(html: &str) -> String {
    html2md::parse_html(html)
}

/// Strip scripts/handlers and dangerous markup from HTML.
pub fn sanitize(html: &str) -> String {
    ammonia::clean(html)
}

/// A stricter sanitize for the *server-side render* path (PDF/DOCX/publish).
///
/// `sanitize()` already drops scripts and all resource-loading tags except
/// `<img>` — but a remote `<img src="http://internal/…">` would make the
/// headless browser (or Pandoc) issue a request *from the server*, an SSRF /
/// data-exfiltration vector. This pass neutralizes any non-`data:` image source
/// (remote or relative) so a render never touches the network for page content.
/// Combined with the renderer's DNS block (see `html_to_pdf`), embedded `data:`
/// images still work; everything else is inert.
pub fn harden_for_render(html: &str) -> String {
    use std::collections::HashSet;
    let mut builder = ammonia::Builder::default();
    // Allow `data:` so embedded images survive (ammonia drops it by default);
    // the attribute filter below still removes any non-`data:` image source, so
    // a remote/relative `<img>` never triggers a fetch. `<a href>` links are
    // kept — they cause no request during a render.
    builder.url_schemes(HashSet::from(["http", "https", "mailto", "tel", "data"]));
    builder.attribute_filter(|element, attribute, value| {
        if element == "img"
            && (attribute == "src" || attribute == "srcset")
            && !value.starts_with("data:")
        {
            // Drop the attribute entirely → no fetch, no broken-network hang.
            None
        } else {
            Some(value.into())
        }
    });
    builder.clean(html).to_string()
}

/// Convert `content` (in `source` format) to `target`, returning file bytes.
pub async fn export(
    content: &str,
    source: SourceFormat,
    target: TargetFormat,
) -> Result<Vec<u8>, ConvertError> {
    // Normalize to sanitized HTML as the common pivot.
    let html = match source {
        SourceFormat::Markdown => md_to_html(content),
        SourceFormat::Html => content.to_string(),
    };
    let html = sanitize(&html);

    match target {
        TargetFormat::Html => Ok(html.into_bytes()),
        TargetFormat::Markdown => {
            // Round-tripping md→html→md loses fidelity, so keep the original
            // when the source was already Markdown.
            let md = if source == SourceFormat::Markdown {
                content.to_string()
            } else {
                html_to_md(&html)
            };
            Ok(md.into_bytes())
        }
        TargetFormat::Pdf => html_to_pdf(&html).await,
        TargetFormat::Docx => html_to_docx(&html).await,
    }
}

const PRINT_CSS: &str = "body{font-family:-apple-system,system-ui,'Segoe UI',Roboto,sans-serif;\
line-height:1.6;color:#111;max-width:48rem;margin:2.5rem auto;padding:0 1rem}\
h1,h2,h3{line-height:1.25;margin-top:1.4em}\
pre{background:#f4f4f5;padding:.75rem;border-radius:6px;overflow:auto}\
code{background:#f4f4f5;padding:.1rem .3rem;border-radius:4px}\
pre code{background:none;padding:0}\
blockquote{border-left:3px solid #d4d4d8;margin:0;padding-left:1rem;color:#52525b}\
table{border-collapse:collapse}th,td{border:1px solid #d4d4d8;padding:.4rem .6rem}\
img{max-width:100%}";

async fn html_to_pdf(body_html: &str) -> Result<Vec<u8>, ConvertError> {
    let chrome = std::env::var("CHROME_BIN").unwrap_or_else(|_| "google-chrome-stable".to_string());
    let id = Uuid::new_v4().to_string();
    let dir = std::env::temp_dir();
    let in_path = dir.join(format!("baitler-export-{id}.html"));
    let out_path = dir.join(format!("baitler-export-{id}.pdf"));

    // SSRF defense: strip remote image sources so the render never fetches page
    // content over the network.
    let safe_body = harden_for_render(body_html);
    let document = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{PRINT_CSS}</style></head>\
         <body>{safe_body}</body></html>"
    );
    tokio::fs::write(&in_path, document)
        .await
        .map_err(|e| ConvertError::Failed(e.to_string()))?;

    let mut cmd = tokio::process::Command::new(&chrome);
    cmd.arg("--headless=new")
        .arg("--disable-gpu")
        // Defense-in-depth: make every hostname fail to resolve, so even a
        // missed remote reference can't reach an internal service. IP-literal
        // SSRF still needs OS-level isolation — see CHROME notes in .env.example.
        .arg("--host-resolver-rules=MAP * ~NOTFOUND")
        .arg("--disable-background-networking")
        .arg("--no-first-run")
        .arg("--no-pdf-header-footer")
        .arg(format!("--print-to-pdf={}", out_path.display()))
        .arg(format!("file://{}", in_path.display()));
    // `--no-sandbox` is required when Chrome runs as root (containers/CI) but
    // weakens isolation; keep it on by default, allow hardened hosts to drop it.
    if std::env::var("CHROME_NO_SANDBOX").as_deref() != Ok("false") {
        cmd.arg("--no-sandbox");
    }

    let run = tokio::time::timeout(Duration::from_secs(30), cmd.output()).await;

    let _ = tokio::fs::remove_file(&in_path).await;

    let output = match run {
        Ok(Ok(output)) => output,
        Ok(Err(_)) => {
            let _ = tokio::fs::remove_file(&out_path).await;
            return Err(ConvertError::NotConfigured("PDF"));
        }
        Err(_) => {
            let _ = tokio::fs::remove_file(&out_path).await;
            return Err(ConvertError::Failed("PDF render timed out".to_string()));
        }
    };
    if !output.status.success() {
        let _ = tokio::fs::remove_file(&out_path).await;
        return Err(ConvertError::Failed(format!(
            "PDF renderer exited with {}",
            output.status
        )));
    }

    let bytes = tokio::fs::read(&out_path)
        .await
        .map_err(|e| ConvertError::Failed(e.to_string()))?;
    let _ = tokio::fs::remove_file(&out_path).await;

    if !bytes.starts_with(b"%PDF") {
        return Err(ConvertError::Failed(
            "renderer did not produce a PDF".to_string(),
        ));
    }
    Ok(bytes)
}

async fn html_to_docx(body_html: &str) -> Result<Vec<u8>, ConvertError> {
    let pandoc = std::env::var("PANDOC_BIN").unwrap_or_else(|_| "pandoc".to_string());
    let id = Uuid::new_v4().to_string();
    let dir = std::env::temp_dir();
    let in_path = dir.join(format!("baitler-export-{id}.html"));
    let out_path = dir.join(format!("baitler-export-{id}.docx"));

    // Same SSRF defense as the PDF path: Pandoc would otherwise fetch remote
    // images while building the .docx. `--sandbox` blocks its file/network IO.
    let safe_body = harden_for_render(body_html);
    tokio::fs::write(&in_path, safe_body)
        .await
        .map_err(|e| ConvertError::Failed(e.to_string()))?;

    let run = tokio::process::Command::new(&pandoc)
        .arg("--sandbox")
        .arg("-f")
        .arg("html")
        .arg("-t")
        .arg("docx")
        .arg("-o")
        .arg(&out_path)
        .arg(&in_path)
        .output()
        .await;

    let _ = tokio::fs::remove_file(&in_path).await;

    match run {
        Ok(output) if output.status.success() => {
            let bytes = tokio::fs::read(&out_path)
                .await
                .map_err(|e| ConvertError::Failed(e.to_string()))?;
            let _ = tokio::fs::remove_file(&out_path).await;
            Ok(bytes)
        }
        Ok(_) => {
            let _ = tokio::fs::remove_file(&out_path).await;
            Err(ConvertError::Failed("Word conversion failed".to_string()))
        }
        // Binary not found → not configured on this server.
        Err(_) => Err(ConvertError::NotConfigured("Word")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_to_html_renders_gfm() {
        let html = md_to_html("# Title\n\n- a\n- b\n");
        assert!(html.contains("<h1>"));
        assert!(html.contains("<li>a</li>"));
    }

    #[test]
    fn sanitize_strips_scripts() {
        let dirty = "<p>ok</p><script>alert(1)</script><img src=x onerror=alert(1)>";
        let clean = sanitize(dirty);
        assert!(!clean.contains("<script"));
        assert!(!clean.contains("onerror"));
        assert!(clean.contains("<p>ok</p>"));
    }

    #[test]
    fn html_to_markdown_round_trips_basics() {
        let md = html_to_md("<h1>Hi</h1><p>hello <strong>world</strong></p>");
        assert!(md.contains("Hi"));
        assert!(md.contains("**world**"));
    }

    #[test]
    fn harden_strips_remote_images_keeps_data_and_text() {
        let dirty = "<p>hi</p>\
            <img src=\"http://169.254.169.254/latest/meta-data/\">\
            <img src=\"//evil.test/x.png\">\
            <img src=\"/local/rel.png\">\
            <img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"ok\">\
            <a href=\"http://example.com\">link</a>";
        let safe = harden_for_render(dirty);
        // No remote/relative image source survives → renderer makes no request.
        assert!(
            !safe.contains("169.254.169.254"),
            "internal SSRF target leaked"
        );
        assert!(!safe.contains("evil.test"));
        assert!(!safe.contains("/local/rel.png"));
        // Embedded data: image and plain text/links are preserved.
        assert!(safe.contains("data:image/png;base64"));
        assert!(safe.contains("<p>hi</p>"));
        assert!(
            safe.contains("example.com"),
            "hyperlinks (no fetch) are kept"
        );
    }

    #[test]
    fn harden_strips_resource_loading_tags() {
        // iframe/object/embed/link/script are not in the allowlist at all.
        let dirty = "<iframe src=\"http://internal/\"></iframe>\
            <link rel=stylesheet href=\"http://internal/x.css\">\
            <object data=\"http://internal/y\"></object><p>body</p>";
        let safe = harden_for_render(dirty);
        assert!(!safe.contains("internal"));
        assert!(safe.contains("<p>body</p>"));
    }
}
