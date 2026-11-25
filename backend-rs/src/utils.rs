//! Shared helpers.

use ammonia::Builder;

/// HTML sanitization matching Go `utils.EscapeHTML` (bluemonday UGCPolicy +
/// `InitBluemonday` extensions): nil/empty input → empty string; otherwise
/// strip everything outside the allow-list.
///
/// Parity notes:
/// - bluemonday's UGC tag set ⊇ ammonia's default tags, so start from
///   `Builder::default()` and only add the Go-policy extras.
/// - `data-*` attributes allowed globally (Go `AllowDataAttributes`): the
///   frontend embeds `data-source-line`, `data-internal-link`,
///   `data-checkbox-index`, `data-src` (pdf-embed), `data-mermaid-source`…
/// - media elements carry `preload` + inline sizing `style` beyond Go's
///   `src/controls/alt` (markdown-it media renderer).
/// - KaTeX/MathML + SVG allow-lists mirror Go; without them math and svg
///   embeds are stripped on save.
///
/// # Examples
///
/// ```
/// use pergame_rs::utils::sanitize_html;
///
/// assert_eq!(sanitize_html(None), "");
/// assert_eq!(sanitize_html(Some("")), "");
/// let clean = sanitize_html(Some("<p>hi</p><script>alert(1)</script>"));
/// assert!(clean.contains("hi"));
/// assert!(!clean.contains("script"));
/// ```
pub fn sanitize_html(input: Option<&str>) -> String {
    let Some(s) = input else {
        return String::new();
    };
    if s.is_empty() {
        return String::new();
    }
    let mut b = Builder::new();
    // `style` is allow-listed for SVG embeds (Go policy); keep its contents.
    b.rm_clean_content_tags(&["style"]);
    b.add_tags(&[
        // Media (Go) + pdf/svg embeds
        "audio",
        "video",
        "object",
        "iframe",
        "svg",
        "g",
        "path",
        "line",
        "polygon",
        "polyline",
        "rect",
        "circle",
        "ellipse",
        "text",
        "style", //
        // Forms: markdown checkbox (Go)
        "input",
        "textarea",
        "button",
        "label",
        "select",
        "option",
        "fieldset",
        "legend", //
        // Frontend custom element (Go)
        "tag", //
        // MathML (Go)
        "math",
        "maction",
        "annotation",
        "annotation-xml",
        "menclose",
        "merror",
        "mfenced",
        "mfrac",
        "mi",
        "mmultiscripts",
        "mn",
        "mo",
        "mover",
        "mpadded",
        "mphantom",
        "mprescripts",
        "mroot",
        "mrow",
        "ms",
        "semantics",
        "mspace",
        "msqrt",
        "mstyle",
        "msub",
        "msup",
        "msubsup",
        "mtable",
        "mtd",
        "mtext",
        "mtr",
        "mtimes",
        "munder",
        "munderover",
    ]);
    // Global attributes (Go) + data-* prefix.
    b.add_generic_attributes(&[
        "class",
        "id",
        "style",
        "tabindex",
        "aria-hidden",
        "display",
        "xmlns",
        "encoding",
        "accent",
        "width",
        "height",
        "type",
        "title",
        // Fork addition (no Go counterpart): credentialed media fetches for
        // the authed /api/resources/:id/file reads.
        "crossorigin",
    ]);
    b.add_generic_attribute_prefixes(&["data-"]);
    // Media / embeds.
    b.add_tag_attributes("audio", &["src", "controls", "alt", "preload"]);
    b.add_tag_attributes("video", &["src", "controls", "alt", "preload"]);
    b.add_tag_attributes("img", &["src", "alt", "title", "width", "height", "data"]);
    b.add_tag_attributes(
        "object",
        &[
            "data",
            "type",
            "aria-label",
            "src",
            "alt",
            "title",
            "width",
            "height",
        ],
    );
    b.add_tag_attributes(
        "iframe",
        &[
            "src",
            "width",
            "height",
            "allow",
            "allowfullscreen",
            "frameborder",
        ],
    );
    // Links: keep rel as-is (Go disables nofollow).
    b.add_tag_attributes("a", &["href", "rel"]);
    b.link_rel(None);
    // Forms.
    b.add_tag_attributes("input", &["checked", "type"]);
    // SVG presentation attributes (Go).
    b.add_tag_attributes(
        "svg",
        &[
            "viewBox",
            "fill-rule",
            "d",
            "x",
            "y",
            "cx",
            "cy",
            "r",
            "rx",
            "ry",
            "points",
            "stroke",
            "stroke-width",
            "fill",
            "preserveAspectRatio",
            "xmlns",
            "version",
            "transform",
            "x1",
            "y1",
            "x2",
            "y2",
        ],
    );
    for tag in [
        "path", "line", "polygon", "polyline", "rect", "circle", "ellipse", "text", "g",
    ] {
        b.add_tag_attributes(
            tag,
            &[
                "viewBox",
                "fill-rule",
                "d",
                "x",
                "y",
                "cx",
                "cy",
                "r",
                "rx",
                "ry",
                "points",
                "stroke",
                "stroke-width",
                "fill",
                "preserveAspectRatio",
                "transform",
                "x1",
                "y1",
                "x2",
                "y2",
            ],
        );
    }
    b.clean(s).to_string()
}

/// Source-code / text-file extensions accepted as resources even when the
/// browser sends no useful Content-Type. Frontend mirrors this list in
/// `frontend/app/helpers/resources.ts` (`CODE_EXTS`) for markdown embed
/// dispatch — keep both in sync.
pub const CODE_EXTS: &[&str] = &[
    "py",
    "rs",
    "go",
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "java",
    "c",
    "h",
    "cpp",
    "hpp",
    "cs",
    "rb",
    "php",
    "sh",
    "bash",
    "zsh",
    "fish",
    "sql",
    "r",
    "swift",
    "kt",
    "scala",
    "pl",
    "lua",
    "toml",
    "yaml",
    "yml",
    "json",
    "xml",
    "xsd",
    "ini",
    "md",
    "markdown",
    "txt",
    "csv",
    "diff",
    "log",
    "dockerfile",
    "makefile",
    "bat",
    "ps1",
    "vim",
    "el",
];

/// Lowercased extension of `filename` (without the dot), empty when none.
pub fn file_ext(filename: &str) -> String {
    let name = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext.to_ascii_lowercase(),
        // Extensionless well-known names (Makefile, Dockerfile).
        _ if CODE_EXTS.iter().any(|e| e.eq_ignore_ascii_case(name)) => name.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// A file is text/source when its extension is a known code extension or the
/// mime is `text/*`.
pub fn is_text_resource(filename: &str, filetype: &str) -> bool {
    CODE_EXTS.contains(&file_ext(filename).as_str()) || filetype.starts_with("text/")
}

/// Best-effort mime for binary uploads that omit `content_type`
/// (mirrors the REST allowlist families: images, video, audio, PDF,
/// office docs, archives).
pub fn guess_mime(filename: &str) -> Option<&'static str> {
    Some(match file_ext(filename).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "ico" => "image/vnd.microsoft.icon",
        "mp4" => "video/mp4",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "weba" => "audio/webm",
        "zip" => "application/zip",
        "rar" => "application/x-rar-compressed",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "bz2" => "application/x-bzip2",
        _ => return None,
    })
}

#[cfg(test)]
mod code_ext_tests {
    use super::*;

    #[test]
    fn file_ext_extracts_and_normalizes() {
        assert_eq!(file_ext("a/b/c.PY"), "py");
        assert_eq!(file_ext("main.rs"), "rs");
        assert_eq!(file_ext("no_ext"), "");
        assert_eq!(file_ext(".hidden"), "");
        assert_eq!(file_ext("Makefile"), "makefile");
        assert_eq!(file_ext("archive.tar.gz"), "gz");
    }

    #[test]
    fn text_resource_by_ext_or_mime() {
        assert!(is_text_resource("x.py", "application/octet-stream"));
        assert!(is_text_resource("blob.bin", "text/plain"));
        assert!(!is_text_resource("blob.bin", "application/octet-stream"));
        assert!(!is_text_resource("x.png", "image/png"));
    }

    #[test]
    fn mime_guess_covers_allowlist_families() {
        assert_eq!(guess_mime("a.PNG"), Some("image/png"));
        assert_eq!(guess_mime("clip.mov"), Some("video/quicktime"));
        assert_eq!(guess_mime("s.aac"), Some("audio/aac"));
        assert_eq!(guess_mime("doc.pdf"), Some("application/pdf"));
        assert_eq!(guess_mime("s.xlsx"), Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
        assert_eq!(guess_mime("a.zip"), Some("application/zip"));
        assert_eq!(guess_mime("n.csv"), Some("text/csv"));
        assert_eq!(guess_mime("blob.bin"), None);
        assert_eq!(guess_mime("noext"), None);
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use super::sanitize_html;

    proptest! {
        /// Sanitization never panics and never emits active content, no
        /// matter the input bytes.
        #[test]
        fn no_active_content_survives(s in "\\PC{0,256}") {
            let out = sanitize_html(Some(&s));
            prop_assert!(!out.contains("<script"));
            prop_assert!(!out.contains("javascript:"));
            prop_assert!(!out.to_lowercase().contains("onerror="));
        }

        /// Plain text without markup passes through unchanged (modulo
        /// entity escaping of `&<>"`).
        #[test]
        fn plain_text_roundtrips(s in "[a-zA-Z0-9 .,;:!?()\\-]{1,128}") {
            prop_assert_eq!(sanitize_html(Some(&s)), s.replace('&', "&amp;"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_html;

    #[test]
    fn keeps_media_embeds() {
        let out = sanitize_html(Some(
            r#"<video controls preload="metadata" style="max-width:100%" src="https://x/a.mp4#t=10,20">demo</video>
<audio controls preload="metadata" src="https://x/a.mp3">demo</audio>
<div class="pdf-embed" data-src="https://x/a.pdf#page=3" data-alt="demo"></div>
<object data="https://x/a.svg" type="image/svg+xml" aria-label="demo">demo</object>"#,
        ));
        for keep in [
            "<video",
            "preload=\"metadata\"",
            "src=\"https://x/a.mp4#t=10,20\"",
            "<audio",
            "class=\"pdf-embed\"",
            "data-src=\"https://x/a.pdf#page=3\"",
            "<object",
            "data=\"https://x/a.svg\"",
        ] {
            assert!(out.contains(keep), "missing {keep} in {out}");
        }
    }

    #[test]
    fn keeps_frontend_html_widgets() {
        let out = sanitize_html(Some(
            r#"<label class="checkbox-container"><input type="checkbox" checked data-checkbox-index="0"><span>x</span></label>
<a href="/dashboard/docs/1" class="internal-link" data-internal-link="1">doc</a>
<span style="color:red">c</span><p data-source-line="0">t</p>"#,
        ));
        for keep in [
            "<input",
            "checked",
            "data-checkbox-index=\"0\"",
            "data-internal-link=\"1\"",
            "style=\"color:red\"",
            "data-source-line=\"0\"",
        ] {
            assert!(out.contains(keep), "missing {keep} in {out}");
        }
    }

    #[test]
    fn strips_active_content() {
        let out = sanitize_html(Some(
            r#"<script>alert(1)</script><p onclick="x()">t</p><a href="javascript:alert(1)">j</a>"#,
        ));
        assert!(!out.contains("script"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
        assert!(!out.contains("javascript:"), "{out}");
    }
}
