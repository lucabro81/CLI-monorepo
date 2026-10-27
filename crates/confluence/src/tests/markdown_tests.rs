#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fixtures are shaped like Confluence's `export_view` body (issue #254):
//! macros already rendered to HTML, with Confluence's own classes and wrappers.

use super::html_to_markdown;

fn md(html: &str) -> String {
    html_to_markdown(html).unwrap()
}

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
        // Column width is measured before `|` is escaped: still a valid GFM table.
        "| **Name** | Owner  |\n| -------- | ------ |\n| api      | team a |\n| web      | x &#124; y |"
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
fn an_info_panel_keeps_its_text_and_drops_the_icon() {
    let html = r#"<div class="confluence-information-macro confluence-information-macro-information conf-macro output-block" data-hasbody="true" data-macro-name="info"><span class="aui-icon aui-icon-small aui-iconfont-info confluence-information-macro-icon"> </span><div class="confluence-information-macro-body"><p>Read this first.</p></div></div>"#;

    assert_eq!(md(html), "Read this first.");
}

#[test]
fn a_status_lozenge_becomes_its_text() {
    let html = r#"<p>State: <span class="status-macro aui-lozenge aui-lozenge-success conf-macro output-inline" data-hasbody="false" data-macro-name="status">DONE</span></p>"#;

    assert_eq!(md(html), "State: DONE");
}

#[test]
fn an_embedded_image_becomes_a_markdown_image() {
    let html = r#"<p><span class="confluence-embedded-file-wrapper"><img class="confluence-embedded-image" src="https://example.atlassian.net/wiki/download/attachments/1/diagram.png?api=v2" alt="diagram"/></span></p>"#;

    assert_eq!(md(html), "![diagram](https://example.atlassian.net/wiki/download/attachments/1/diagram.png?api=v2)");
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
fn script_and_style_content_is_dropped() {
    let html = r"<p>Kept</p><script>alert(1)</script><style>p { color: red }</style>";

    assert_eq!(md(html), "Kept");
}

#[test]
fn an_empty_or_blank_body_is_an_empty_string() {
    assert_eq!(md(""), "");
    assert_eq!(md("<p> </p>"), "");
}
