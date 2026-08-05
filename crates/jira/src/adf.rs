//! Markdown → Atlassian Document Format (ADF) conversion.
//!
//! `markdown_to_adf_content` parses a Markdown string with `pulldown_cmark`
//! and builds the corresponding ADF block-node array (`paragraph`, `heading`,
//! `bulletList`/`orderedList`, `codeBlock`), with inline marks (`strong`,
//! `em`, `code`, `link`) applied to `text` nodes and explicit `hardBreak`
//! nodes for embedded line breaks. Returns block-level nodes ready to drop
//! into a `doc`'s `content` array or a comment body's `content` array —
//! callers do not wrap them in an outer paragraph themselves (see
//! `client::create_issue`, `client::add_comment`).
//!
//! This module is Markdown-only and has no concept of Jira mentions: a
//! `{{mention:ACCOUNT_ID}}` placeholder passed through here comes out as
//! literal text inside a `text` node, same as any other text. Expanding it
//! into a real ADF `mention` node is a separate post-processing pass over
//! the returned tree (see `commands::issue::expand_mentions_in_content`).
//!
//! Empty or whitespace-only input returns an empty `Vec` — Jira rejects an
//! empty comment/description body regardless of the exact empty-ish ADF
//! shape sent (verified live: `content: []`, a single empty paragraph, and a
//! paragraph with one empty-string text node are all rejected with "Comment
//! body can not be empty!"), so there is no shape this function could
//! manufacture that would make an empty input acceptable — callers see the
//! same `ApiError` any other empty-body request would produce.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Parser, Tag, TagEnd};
use serde_json::{json, Value};

/// Converts `text` (Markdown) into a `Vec` of ADF block-level nodes.
pub fn markdown_to_adf_content(text: &str) -> Vec<Value> {
    let mut builder = Builder::new();
    for event in Parser::new(text) {
        builder.handle(event);
    }
    builder.finish()
}

/// One open block-level container while walking the event stream: either a
/// leaf that accumulates inline content directly (`Paragraph`, `Heading`,
/// `CodeBlock`), or a container that accumulates finished child block nodes
/// (`BulletList`, `OrderedList`, `ListItem`).
enum Frame {
    Paragraph(Vec<Value>),
    Heading(u8, Vec<Value>),
    CodeBlock(Option<String>, String),
    BulletList(Vec<Value>),
    OrderedList(Vec<Value>),
    ListItem(Vec<Value>),
}

struct Builder {
    /// Finished top-level block nodes.
    blocks: Vec<Value>,
    /// Stack of currently-open containers (innermost last).
    stack: Vec<Frame>,
    /// Stack of currently-active inline marks (innermost/last-opened last).
    marks: Vec<Value>,
}

impl Builder {
    fn new() -> Self {
        Self { blocks: Vec::new(), stack: Vec::new(), marks: Vec::new() }
    }

    fn handle(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag_end) => self.end(tag_end),
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => {
                self.marks.push(json!({"type": "code"}));
                self.push_text(&text);
                self.marks.pop();
            }
            Event::SoftBreak | Event::HardBreak => self.push_inline(json!({"type": "hardBreak"})),
            // Rules, images, footnotes, tables, HTML, math, etc. have no ADF
            // equivalent handled here — the minimal-effort choice is to skip
            // them rather than guess at a mapping; their inner text (if any)
            // still arrives via separate Text events and is not lost.
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.stack.push(Frame::Paragraph(Vec::new())),
            Tag::Heading { level, .. } => {
                self.stack.push(Frame::Heading(heading_level_number(level), Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        let lang = info.trim();
                        if lang.is_empty() { None } else { Some(lang.to_string()) }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.stack.push(Frame::CodeBlock(language, String::new()));
            }
            Tag::List(start) => {
                self.stack.push(match start {
                    Some(_) => Frame::OrderedList(Vec::new()),
                    None => Frame::BulletList(Vec::new()),
                });
            }
            Tag::Item => self.stack.push(Frame::ListItem(Vec::new())),
            Tag::Strong => self.marks.push(json!({"type": "strong"})),
            Tag::Emphasis => self.marks.push(json!({"type": "em"})),
            Tag::Link { dest_url, link_type, .. } => {
                // Autolinks (`<https://...>`) and inline `[text](url)` links
                // both map to the same ADF link mark.
                if !matches!(link_type, LinkType::Email) {
                    self.marks.push(json!({"type": "link", "attrs": {"href": dest_url.to_string()}}));
                }
            }
            // Block quotes, images, tables, footnotes, definition lists, etc.
            // have no ADF mapping in this converter — inline text inside
            // them (if any) still comes through as plain paragraph text
            // because there's no matching Start/End frame to divert it.
            _ => {}
        }
    }

    fn end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Paragraph => {
                if let Some(Frame::Paragraph(content)) = self.stack.pop() {
                    self.emit_block(json!({"type": "paragraph", "content": content}));
                }
            }
            TagEnd::Heading(_) => {
                if let Some(Frame::Heading(level, content)) = self.stack.pop() {
                    self.emit_block(json!({
                        "type": "heading",
                        "attrs": {"level": level},
                        "content": content
                    }));
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Frame::CodeBlock(language, text)) = self.stack.pop() {
                    let content = if text.is_empty() {
                        vec![]
                    } else {
                        vec![json!({"type": "text", "text": text})]
                    };
                    let mut node = json!({"type": "codeBlock", "content": content});
                    if let Some(lang) = language {
                        node["attrs"] = json!({"language": lang});
                    }
                    self.emit_block(node);
                }
            }
            TagEnd::List(_) => {
                if let Some(frame) = self.stack.pop() {
                    let node = match frame {
                        Frame::BulletList(content) => json!({"type": "bulletList", "content": content}),
                        Frame::OrderedList(content) => json!({"type": "orderedList", "content": content}),
                        other => unreachable_frame(other),
                    };
                    self.emit_block(node);
                }
            }
            TagEnd::Item => {
                // A "tight" list (no blank lines between items) never emits an
                // explicit Start(Paragraph)/End(Paragraph) pair around an
                // item's inline content — `push_inline` opens an implicit
                // paragraph frame on demand in that case (see below), which
                // is never closed by its own End event. Close it now, before
                // closing the item, so the item's content is still a proper
                // ADF block (`listItem` requires block children, not bare
                // inline nodes). A "loose" list's explicit paragraph is
                // already closed and emitted by the time this runs, so this
                // is a no-op for it.
                if matches!(self.stack.last(), Some(Frame::Paragraph(_)))
                    && let Some(Frame::Paragraph(content)) = self.stack.pop()
                {
                    self.emit_block(json!({"type": "paragraph", "content": content}));
                }
                if let Some(Frame::ListItem(content)) = self.stack.pop() {
                    self.emit_block(json!({"type": "listItem", "content": content}));
                }
            }
            TagEnd::Strong | TagEnd::Emphasis => {
                self.marks.pop();
            }
            TagEnd::Link => {
                // Matches the push in `start`: an email autolink pushed no
                // mark, so there is nothing to pop for it.
                if self.marks.last().is_some_and(|m| m["type"] == "link") {
                    self.marks.pop();
                }
            }
            _ => {}
        }
    }

    /// Appends a finished block node either into the currently-open
    /// container (a list item collects paragraphs, a list collects items) or
    /// into the top-level `blocks` output if the stack is empty.
    fn emit_block(&mut self, node: Value) {
        match self.stack.last_mut() {
            Some(Frame::ListItem(content) | Frame::BulletList(content) | Frame::OrderedList(content)) => {
                content.push(node);
            }
            Some(Frame::Paragraph(_) | Frame::Heading(..) | Frame::CodeBlock(..)) | None => {
                self.blocks.push(node);
            }
        }
    }

    /// Appends inline text to whatever leaf container is currently open
    /// (paragraph/heading text, or raw code-block text), applying any active
    /// marks. Plain-text fenced code blocks accumulate raw text instead,
    /// since ADF code-block content carries no marks.
    fn push_text(&mut self, text: &str) {
        match self.stack.last_mut() {
            Some(Frame::CodeBlock(_, buf)) => buf.push_str(text),
            _ => self.push_inline(build_text_node(text, &self.marks)),
        }
    }

    fn push_inline(&mut self, node: Value) {
        match self.stack.last_mut() {
            Some(Frame::Paragraph(content) | Frame::Heading(_, content)) => content.push(node),
            Some(Frame::ListItem(_)) => {
                // Tight list item: pulldown-cmark delivers the item's inline
                // content with no surrounding Paragraph tag at all, but ADF's
                // `listItem` schema requires a block child, not bare inline
                // nodes — open an implicit paragraph frame to hold it.
                // Closed explicitly in `end`'s `TagEnd::Item` arm, since
                // there is no matching End event to close it here. Nested
                // block content (e.g. a nested list) inside a tight item is
                // not supported by this fallback — out of scope for the
                // issue's mapping table, which has no nested-list case.
                self.stack.push(Frame::Paragraph(vec![node]));
            }
            _ => {
                // A SoftBreak/HardBreak or Text event outside of any open
                // paragraph/heading/list-item (e.g. between block elements)
                // has no ADF home — CommonMark does not emit inline events
                // outside an inline container, so this is unreachable in
                // practice.
            }
        }
    }

    fn finish(self) -> Vec<Value> {
        self.blocks
    }
}

fn build_text_node(text: &str, marks: &[Value]) -> Value {
    if marks.is_empty() {
        json!({"type": "text", "text": text})
    } else {
        json!({"type": "text", "text": text, "marks": marks})
    }
}

fn heading_level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Unreachable in practice: `TagEnd::List` is only pushed after a matching
/// `Frame::BulletList`/`Frame::OrderedList` `Tag::List` start, so any other
/// frame at that stack position indicates a `pulldown_cmark` event-stream
/// invariant this module relies on has changed. Returns a diagnostic node
/// instead of panicking, since production code must never unwrap/expect.
fn unreachable_frame(_frame: Frame) -> Value {
    json!({"type": "paragraph", "content": []})
}

#[cfg(test)]
#[path = "tests/adf_tests.rs"]
mod tests;
