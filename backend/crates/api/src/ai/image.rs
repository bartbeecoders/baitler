//! Image generation for canvas "image" parts.
//!
//! Only the offline `mock` provider is wired today — it renders the prompt into
//! an SVG placeholder so the feature is end-to-end testable without egress.
//! Real providers (OpenAI Images, fal.ai) plug in behind the owner's stored API
//! key, mirroring the text-chat provider pattern.

pub const MAX_PROMPT: usize = 2_000;

pub struct GeneratedImage {
    /// A renderable `data:` URL (or, for future providers, a remote https URL).
    pub data_url: String,
    pub mime: String,
    pub provider: String,
}

/// Render the prompt into a labelled SVG placeholder card.
pub fn generate_mock(prompt: &str) -> GeneratedImage {
    let svg = placeholder_svg(prompt);
    GeneratedImage {
        data_url: svg_data_url(&svg),
        mime: "image/svg+xml".into(),
        provider: "mock".into(),
    }
}

fn placeholder_svg(prompt: &str) -> String {
    let mut texts = String::new();
    let mut y = 150;
    for line in wrap(prompt, 30, 6) {
        texts.push_str(&format!(
            "<text x=\"40\" y=\"{y}\" font-family=\"system-ui,-apple-system,sans-serif\" \
             font-size=\"20\" fill=\"#e2e8f0\">{}</text>",
            xml_escape(&line)
        ));
        y += 30;
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"512\" height=\"320\" \
         viewBox=\"0 0 512 320\">\
         <defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"#6366f1\"/><stop offset=\"1\" stop-color=\"#0ea5e9\"/>\
         </linearGradient></defs>\
         <rect width=\"512\" height=\"320\" fill=\"url(#g)\"/>\
         <rect x=\"20\" y=\"20\" width=\"472\" height=\"280\" rx=\"16\" fill=\"#0f172a\" opacity=\"0.55\"/>\
         <text x=\"40\" y=\"70\" font-family=\"system-ui,sans-serif\" font-size=\"13\" \
         letter-spacing=\"2\" fill=\"#94a3b8\">AI PLACEHOLDER IMAGE</text>{texts}</svg>"
    )
}

/// Word-wrap to at most `max_lines` lines of roughly `width` characters.
fn wrap(prompt: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in prompt.split_whitespace() {
        if cur.is_empty() {
            cur = word.chars().take(width).collect();
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            if lines.len() >= max_lines {
                return lines;
            }
            cur = word.chars().take(width).collect();
        }
    }
    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    lines
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Percent-encode an SVG into a `data:` URL safe for `<img src>`.
fn svg_data_url(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len() * 2);
    out.push_str("data:image/svg+xml,");
    for b in svg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_image_is_an_svg_data_url() {
        let img = generate_mock("a red bicycle by the sea");
        assert_eq!(img.mime, "image/svg+xml");
        assert!(img.data_url.starts_with("data:image/svg+xml,"));
        assert!(img.data_url.len() > 100);
    }

    #[test]
    fn wrap_escapes_and_limits() {
        let svg = placeholder_svg("<script>alert(1)</script> & friends");
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&amp;") || svg.contains("&lt;"));
    }
}
