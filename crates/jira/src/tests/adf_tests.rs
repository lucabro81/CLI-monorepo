#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::markdown_to_adf_content;
use serde_json::json;

#[test]
fn plain_single_line_text_becomes_one_paragraph_one_text_node() {
    assert_eq!(
        markdown_to_adf_content("just plain text"),
        vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "just plain text"}]
        })]
    );
}

#[test]
fn empty_input_returns_empty_vec() {
    // Jira rejects an empty comment/description body regardless of the exact
    // empty-ish ADF shape sent (verified live: `content: []`, a single empty
    // paragraph, and a paragraph with one empty-string text node are all
    // rejected with "Comment body can not be empty!") — so there is no shape
    // markdown_to_adf_content could produce that would make an empty input
    // acceptable to Jira. It returns the natural empty result instead of
    // manufacturing a placeholder block.
    assert_eq!(markdown_to_adf_content(""), Vec::<serde_json::Value>::new());
}

#[test]
fn whitespace_only_input_returns_empty_vec() {
    assert_eq!(markdown_to_adf_content("   \n\n  "), Vec::<serde_json::Value>::new());
}

#[test]
fn blank_line_separates_two_paragraphs() {
    assert_eq!(
        markdown_to_adf_content("first paragraph\n\nsecond paragraph"),
        vec![
            json!({"type": "paragraph", "content": [{"type": "text", "text": "first paragraph"}]}),
            json!({"type": "paragraph", "content": [{"type": "text", "text": "second paragraph"}]}),
        ]
    );
}

#[test]
fn single_newline_inside_paragraph_becomes_hard_break() {
    assert_eq!(
        markdown_to_adf_content("line one\nline two"),
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "line one"},
                {"type": "hardBreak"},
                {"type": "text", "text": "line two"}
            ]
        })]
    );
}

#[test]
fn explicit_hard_break_two_trailing_spaces_also_becomes_hard_break() {
    assert_eq!(
        markdown_to_adf_content("line one  \nline two"),
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "line one"},
                {"type": "hardBreak"},
                {"type": "text", "text": "line two"}
            ]
        })]
    );
}

#[test]
fn heading_level_1() {
    assert_eq!(
        markdown_to_adf_content("# Title"),
        vec![json!({
            "type": "heading",
            "attrs": {"level": 1},
            "content": [{"type": "text", "text": "Title"}]
        })]
    );
}

#[test]
fn heading_level_6_boundary() {
    assert_eq!(
        markdown_to_adf_content("###### Deepest"),
        vec![json!({
            "type": "heading",
            "attrs": {"level": 6},
            "content": [{"type": "text", "text": "Deepest"}]
        })]
    );
}

#[test]
fn bullet_list_two_items() {
    assert_eq!(
        markdown_to_adf_content("- item1\n- item2"),
        vec![json!({
            "type": "bulletList",
            "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "item1"}]}]},
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "item2"}]}]}
            ]
        })]
    );
}

#[test]
fn bullet_list_with_star_marker() {
    assert_eq!(
        markdown_to_adf_content("* item1\n* item2"),
        vec![json!({
            "type": "bulletList",
            "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "item1"}]}]},
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "item2"}]}]}
            ]
        })]
    );
}

#[test]
fn ordered_list_two_items() {
    assert_eq!(
        markdown_to_adf_content("1. first\n2. second"),
        vec![json!({
            "type": "orderedList",
            "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "first"}]}]},
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "second"}]}]}
            ]
        })]
    );
}

#[test]
fn back_to_back_lists_stay_separate() {
    assert_eq!(
        markdown_to_adf_content("- a\n\n1. b"),
        vec![
            json!({"type": "bulletList", "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "a"}]}]}
            ]}),
            json!({"type": "orderedList", "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "b"}]}]}
            ]}),
        ]
    );
}

#[test]
fn inline_code_span_gets_code_mark() {
    assert_eq!(
        markdown_to_adf_content("run `cargo test` now"),
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "run "},
                {"type": "text", "text": "cargo test", "marks": [{"type": "code"}]},
                {"type": "text", "text": " now"}
            ]
        })]
    );
}

#[test]
fn fenced_code_block_with_language() {
    assert_eq!(
        markdown_to_adf_content("```rust\nfn main() {}\n```"),
        vec![json!({
            "type": "codeBlock",
            "attrs": {"language": "rust"},
            "content": [{"type": "text", "text": "fn main() {}\n"}]
        })]
    );
}

#[test]
fn fenced_code_block_without_language_has_no_language_attr() {
    assert_eq!(
        markdown_to_adf_content("```\nplain block\n```"),
        vec![json!({
            "type": "codeBlock",
            "content": [{"type": "text", "text": "plain block\n"}]
        })]
    );
}

#[test]
fn empty_fenced_code_block() {
    assert_eq!(
        markdown_to_adf_content("```\n```"),
        vec![json!({"type": "codeBlock", "content": []})]
    );
}

#[test]
fn bold_text_gets_strong_mark() {
    assert_eq!(
        markdown_to_adf_content("**bold**"),
        vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "bold", "marks": [{"type": "strong"}]}]
        })]
    );
}

#[test]
fn underscore_italic_gets_em_mark() {
    assert_eq!(
        markdown_to_adf_content("_italic_"),
        vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "italic", "marks": [{"type": "em"}]}]
        })]
    );
}

#[test]
fn asterisk_italic_gets_em_mark() {
    assert_eq!(
        markdown_to_adf_content("*italic*"),
        vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "italic", "marks": [{"type": "em"}]}]
        })]
    );
}

#[test]
fn bold_and_italic_combine_into_two_marks_on_one_text_node() {
    assert_eq!(
        markdown_to_adf_content("**_both_**"),
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "both", "marks": [{"type": "strong"}, {"type": "em"}]}
            ]
        })]
    );
}

#[test]
fn link_gets_link_mark_with_href() {
    assert_eq!(
        markdown_to_adf_content("[docs](https://example.com/path?a=1&b=2)"),
        vec![json!({
            "type": "paragraph",
            "content": [
                {"type": "text", "text": "docs", "marks": [{"type": "link", "attrs": {"href": "https://example.com/path?a=1&b=2"}}]}
            ]
        })]
    );
}

#[test]
fn mention_placeholder_passes_through_as_literal_text() {
    // adf.rs is Markdown-only and mention-agnostic; expanding
    // {{mention:ACCOUNT_ID}} into a real ADF mention node is a separate
    // post-processing pass (commands::issue::expand_mentions_in_content),
    // not this module's concern.
    assert_eq!(
        markdown_to_adf_content("hi {{mention:5b10ac8d}} there"),
        vec![json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": "hi {{mention:5b10ac8d}} there"}]
        })]
    );
}

#[test]
fn heading_paragraph_and_list_together() {
    assert_eq!(
        markdown_to_adf_content("## Title\n\nSome text\n\n- one\n- two"),
        vec![
            json!({"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "Title"}]}),
            json!({"type": "paragraph", "content": [{"type": "text", "text": "Some text"}]}),
            json!({"type": "bulletList", "content": [
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "one"}]}]},
                {"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "two"}]}]}
            ]}),
        ]
    );
}
