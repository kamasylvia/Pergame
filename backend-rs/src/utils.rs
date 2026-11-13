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
