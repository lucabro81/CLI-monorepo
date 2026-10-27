#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fixtures are shaped like Confluence's `export_view` body (issue #254):
//! macros already rendered to HTML, with Confluence's own classes and wrappers.

use super::html_to_markdown;

fn md(html: &str) -> String {
    html_to_markdown(html, None).unwrap()
}

const SITE: &str = "https://example.atlassian.net";

#[test]
fn headings_keep_their_level_and_drop_confluence_anchor_ids() {
    let html = r#"<h1 id="Doc-Intro">Intro</h1><p>Text</p><h2 id="Doc-Sub">Sub</h2>"#;

    assert_eq!(md(html), "# Intro\n\nText\n\n## Sub");
}

#[test]
fn a_confluence_table_with_its_header_row_in_tbody_becomes_a_gfm_table() {
    let html = r#"<div class="table-wrap"><table class="confluenceTable" data-table-width="760"><colgroup><col style="width: 380px;"/><col/></colgroup><tbody><tr><th class="confluenceTh"><p><strong>Name</strong></p></th><th class="confluenceTh"><p>Owner</p></th></tr><tr><td class="confluenceTd"><p>api</p></td><td class="confluenceTd"><p>team a</p></td></tr><tr><td class="confluenceTd"><p>web</p></td><td class="confluenceTd"><p>x | y</p></td></tr></tbody></table></div>"#;

    assert_eq!(
        md(html),
        // Issue #261: `|` in a cell is GFM's `\|`, not htmd's `&#124;`, and the
        // column width counts the escape.
        "| **Name** | Owner  |\n| -------- | ------ |\n| api      | team a |\n| web      | x \\| y |"
    );
}

#[test]
fn the_code_macro_becomes_a_fenced_block_with_the_brush_language() {
    let html = r#"<div class="code panel pdl conf-macro output-block" data-hasbody="true" data-macro-name="code"><div class="codeContent panelContent pdl"><pre class="syntaxhighlighter-pre" data-syntaxhighlighter-params="brush: bash; gutter: false; theme: Confluence" data-theme="Confluence">cargo build
cargo test</pre></div></div>"#;

    assert_eq!(md(html), "```bash\ncargo build\ncargo test\n```");
}

#[test]
fn a_noformat_block_becomes_a_fenced_block_without_language_or_escaping() {
    let html = r#"<div class="preformatted panel"><div class="preformattedContent panelContent"><pre>let a_b = *x* &lt;tag&gt;</pre></div></div>"#;

    assert_eq!(md(html), "```\nlet a_b = *x* <tag>\n```");
}

#[test]
fn a_code_block_containing_a_fence_gets_a_longer_fence() {
    let html = r#"<pre class="syntaxhighlighter-pre" data-syntaxhighlighter-params="brush: text; gutter: false">```
inner
```</pre>"#;

    assert_eq!(md(html), "````text\n```\ninner\n```\n````");
}

#[test]
fn a_pre_code_block_keeps_htmd_handling() {
    let html = r#"<pre><code class="language-rust">fn x() {}</code></pre>"#;

    assert_eq!(md(html), "```rust\nfn x() {}\n```");
}

#[test]
fn panels_become_blockquotes_that_name_their_type() {
    // Issue #261: a warning read as a plain paragraph loses its meaning.
    for (class, macro_name, icon, label) in [
        ("information", "info", "info", "Info"),
        ("note", "note", "warning", "Note"),
        ("warning", "warning", "error", "Warning"),
        ("tip", "tip", "approve", "Tip"),
    ] {
        let html = format!(
            r#"<div class="confluence-information-macro confluence-information-macro-{class} conf-macro output-block" data-hasbody="true" data-macro-name="{macro_name}"><span class="aui-icon aui-icon-small aui-iconfont-{icon} confluence-information-macro-icon"> </span><div class="confluence-information-macro-body"><p>Read this first.</p></div></div>"#
        );

        assert_eq!(md(&html), format!("> **{label}:** Read this first."), "{macro_name} panel");
    }
}

#[test]
fn a_panel_with_several_paragraphs_quotes_every_line() {
    let html = r#"<p>Before</p><div class="confluence-information-macro confluence-information-macro-warning conf-macro output-block" data-macro-name="warning"><span class="aui-icon confluence-information-macro-icon"> </span><div class="confluence-information-macro-body"><p>Not in production.</p><p>Ask the team first.</p></div></div><p>After</p>"#;

    assert_eq!(md(html), "Before\n\n> **Warning:** Not in production.\n>\n> Ask the team first.\n\nAfter");
}

#[test]
fn a_div_of_another_macro_keeps_htmd_handling() {
    let html = r#"<div class="expand-container conf-macro output-block" data-macro-name="expand"><div class="expand-content"><p>Hidden text</p></div></div>"#;

    assert_eq!(md(html), "Hidden text");
}

#[test]
fn a_status_lozenge_becomes_its_text() {
    let html = r#"<p>State: <span class="status-macro aui-lozenge aui-lozenge-success conf-macro output-inline" data-hasbody="false" data-macro-name="status">DONE</span></p>"#;

    assert_eq!(md(html), "State: DONE");
}

#[test]
fn an_embedded_image_becomes_its_alt_text_without_the_attachment_url() {
    // Issue #261: the attachment URL needs authentication, noise for a model.
    let html = r#"<p>See <span class="confluence-embedded-file-wrapper"><img class="confluence-embedded-image" src="https://example.atlassian.net/wiki/download/attachments/1/diagram.png?api=v2" alt="diagram"/></span> above.</p>"#;

    assert_eq!(md(html), "See [image: diagram] above.");
}

#[test]
fn an_image_without_alt_text_is_a_bare_marker() {
    assert_eq!(md(r#"<p><img src="/wiki/download/x.png"/></p>"#), "[image]");
    assert_eq!(md(r#"<p><img src="/wiki/download/x.png" alt=" "/></p>"#), "[image]");
}

#[test]
fn nested_lists_are_indented_under_their_parent_item() {
    let html = r"<ol><li>first<ul><li>sub a</li><li>sub b</li></ul></li><li>second</li></ol>";

    assert_eq!(md(html), "1.  first\n    *   sub a\n    *   sub b\n2.  second");
}

#[test]
fn links_and_inline_code_are_kept() {
    let html = r#"<p>See <a href="https://example.atlassian.net/wiki/spaces/ENG/pages/2/Other" data-linked-resource-id="2">Other page</a> and <code>inline</code>.</p>"#;

    assert_eq!(md(html), "See [Other page](https://example.atlassian.net/wiki/spaces/ENG/pages/2/Other) and `inline`.");
}

#[test]
fn relative_wiki_links_become_absolute_with_the_site_url() {
    // Issue #261: a relative link can't be followed by a model or a formatter.
    let html = r##"<p><a href="/wiki/spaces/ENG/pages/2/Other">Other</a>, <a href="https://example.com/x">ext</a>, <a href="#Doc-Sub">anchor</a></p>"##;

    assert_eq!(
        html_to_markdown(html, Some(SITE)).unwrap(),
        "[Other](https://example.atlassian.net/wiki/spaces/ENG/pages/2/Other), [ext](https://example.com/x), [anchor](#Doc-Sub)"
    );
    assert_eq!(
        html_to_markdown(html, Some("https://example.atlassian.net/")).unwrap(),
        "[Other](https://example.atlassian.net/wiki/spaces/ENG/pages/2/Other), [ext](https://example.com/x), [anchor](#Doc-Sub)",
        "a trailing slash on the site URL is dropped"
    );
}

#[test]
fn without_a_site_url_relative_links_are_left_as_they_are() {
    let html = r#"<p><a href="/wiki/spaces/ENG/pages/2/Other">Other</a></p>"#;

    assert_eq!(md(html), "[Other](/wiki/spaces/ENG/pages/2/Other)");
}

#[test]
fn script_and_style_content_is_dropped() {
    let html = r"<p>Kept</p><script>alert(1)</script><style>p { color: red }</style>";

    assert_eq!(md(html), "Kept");
}

#[test]
fn an_empty_or_blank_body_is_an_empty_string() {
    assert_eq!(md(""), "");
    assert_eq!(md("<p> </p>"), "");
}
