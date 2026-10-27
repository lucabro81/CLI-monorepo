//! HTML → Markdown conversion for page bodies (issue #254), on top of `htmd`.
//!
//! Adjustments to htmd's defaults, all for Confluence's rendered HTML:
//! - `<script>`/`<style>` are dropped (htmd would print their text).
//! - A `<pre>` without a `<code>` child (Confluence's code and noformat macros
//!   render that way) becomes a fenced block of its raw text, with the code
//!   macro's language (`data-syntaxhighlighter-params="brush: bash; ..."`).
//!   htmd would print it as plain, Markdown-escaped text.
//! - Issue #261, for a model reading the result:
//!   - an info/note/warning/tip/success/error panel (`data-macro-name`) is a
//!     blockquote naming its type, `> **Warning:** ...`;
//!   - an image is `[image: <alt>]`, without its attachment URL (it needs
//!     authentication);
//!   - a `|` in a table cell is GFM's `\|`, not htmd's `&#124;`;
//!   - with a site URL, a link to `/wiki/...` becomes absolute.
//! - Storage format (comment bodies, issue #258, [`storage_to_markdown`]):
//!   CDATA is unwrapped first (an HTML parser drops it, and storage keeps macro
//!   text there), `ac:parameter` values are skipped, and a code/noformat body
//!   (`ac:plain-text-body`) is a fenced block. Other `ac:`/`ri:` tags keep
//!   their text only.

use std::rc::Rc;

use htmd::element_handler::{HandlerResult, Handlers};
use htmd::{Element, HtmlToMarkdown, Node};
use markup5ever_rcdom::NodeData;

use crate::error::CliError;

/// Two private-use characters standing for a cell's `|` until htmd has laid
/// out the table: two, like `\|`, so the column widths it measures stay right.
const CELL_PIPE: &str = "\u{E000}\u{E001}";

/// `site_url` is the site's `https://<site>.atlassian.net` (the credentials'
/// `site_url`); `None` leaves relative links as they are.
pub fn html_to_markdown(html: &str, site_url: Option<&str>) -> Result<String, CliError> {
    let site_url = site_url.map(|url| url.trim_end_matches('/').to_string());
    let markdown = HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "ac:parameter"])
        .add_handler(vec!["pre"], pre_handler)
        .add_handler(vec!["ac:plain-text-body"], plain_text_body_handler)
        .add_handler(vec!["div"], panel_handler)
        .add_handler(vec!["img"], image_handler)
        .add_handler(vec!["td", "th"], cell_handler)
        .add_handler(vec!["a"], move |handlers: &dyn Handlers, element: Element| {
            link_handler(handlers, element, site_url.as_deref())
        })
        .build()
        .convert(html)
        // html5ever reads from the in-memory string: its I/O error cannot happen.
        .map_err(|e| CliError::Internal(format!("converting the page body to Markdown failed: {e}")))?;
    Ok(markdown.replace(CELL_PIPE, "\\|"))
}

/// A storage-format body (XHTML with `ac:`/`ri:` macro tags) as Markdown;
/// `site_url` as for [`html_to_markdown`].
pub fn storage_to_markdown(storage: &str, site_url: Option<&str>) -> Result<String, CliError> {
    html_to_markdown(&unwrap_cdata(storage), site_url)
}

/// `<![CDATA[a<b]]>` → `a&lt;b`: the text an HTML parser would otherwise drop.
fn unwrap_cdata(storage: &str) -> String {
    let mut out = String::with_capacity(storage.len());
    let mut rest = storage;
    while let Some(start) = rest.find("<![CDATA[") {
        out.push_str(&rest[..start]);
        let inner = &rest[start + "<![CDATA[".len()..];
        let (text, after) = inner.split_once("]]>").unwrap_or((inner, ""));
        out.push_str(&text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"));
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A storage code/noformat macro body: its raw text as a fenced block (the
/// language sits in an `ac:parameter`, skipped).
// The signature is htmd's handler type.
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
fn plain_text_body_handler(_handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let mut text = String::new();
    collect_text(element.node, &mut text);
    Some(fenced(text.trim_matches('\n'), "").into())
}

/// Panel macros and the label their blockquote starts with.
const PANELS: [(&str, &str); 6] =
    [("info", "Info"), ("note", "Note"), ("warning", "Warning"), ("tip", "Tip"), ("success", "Success"), ("error", "Error")];

/// The older panel markup's `confluence-information-macro-<type>` class, for a
/// panel `div` without `data-macro-name`.
const PANEL_CLASSES: [(&str, &str); 4] = [
    ("confluence-information-macro-information", "Info"),
    ("confluence-information-macro-note", "Note"),
    ("confluence-information-macro-warning", "Warning"),
    ("confluence-information-macro-tip", "Tip"),
];

fn panel_label(element: &Element) -> Option<&'static str> {
    if let Some(name) = attr(element, "data-macro-name") {
        return PANELS.iter().find(|(macro_name, _)| *macro_name == name).map(|(_, label)| *label);
    }
    let classes = attr(element, "class")?;
    PANEL_CLASSES
        .iter()
        .find(|(class, _)| classes.split_whitespace().any(|c| c == *class))
        .map(|(_, label)| *label)
}

fn panel_handler(handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let Some(label) = panel_label(&element) else {
        return handlers.fallback(element);
    };
    // The panel's text, without its icon (an empty <span>, outside the body div).
    let body = find_child_with_class(element.node, "confluence-information-macro-body")
        .unwrap_or_else(|| Rc::clone(element.node));
    let content = handlers.walk_children(&body).content;
    let content = content.trim();
    // A label glued to a list item or a fence would break that block.
    let starts_a_block = content.starts_with(['*', '-', '+', '`', '~', '#', '>', '|'])
        || content.split_once(". ").is_some_and(|(number, _)| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    let text = if content.is_empty() {
        format!("**{label}:**")
    } else if starts_a_block {
        format!("**{label}:**\n\n{content}")
    } else {
        format!("**{label}:** {content}")
    };
    let quoted: Vec<String> = text
        .lines()
        .map(|line| if line.is_empty() { ">".to_string() } else { format!("> {line}") })
        .collect();
    Some(format!("\n\n{}\n\n", quoted.join("\n")).into())
}

// The signature is htmd's handler type.
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
fn image_handler(_handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let alt = attr(&element, "alt").map(str::trim).filter(|alt| !alt.is_empty());
    Some(match alt {
        Some(alt) => format!("[image: {alt}]"),
        None => "[image]".to_string(),
    }
    .into())
}

/// A link to `/wiki/...` made absolute with `site_url`; other links, or no
/// site URL, keep htmd's handling. Done on the element, not on the output
/// text, so code blocks are never touched.
fn link_handler(handlers: &dyn Handlers, element: Element, site_url: Option<&str>) -> Option<HandlerResult> {
    let absolute = match (site_url, attr(&element, "href")) {
        (Some(site_url), Some(href)) if href.starts_with("/wiki/") => format!("{site_url}{href}"),
        _ => return handlers.fallback(element),
    };
    let text = handlers.walk_children(element.node).content;
    // CommonMark: a destination with spaces or parentheses goes in <...>.
    let destination = if absolute.contains([' ', '(', ')']) { format!("<{absolute}>") } else { absolute };
    Some(format!("[{}]({destination})", text.trim()).into())
}

fn cell_handler(handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let mut result = handlers.fallback(element)?;
    result.content = result.content.replace('|', CELL_PIPE);
    Some(result)
}

fn attr<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    element.attrs.iter().find(|attr| &*attr.name.local == name).map(|attr| &*attr.value)
}

/// The first child element of `node` with `class` among its classes.
fn find_child_with_class(node: &Rc<Node>, class: &str) -> Option<Rc<Node>> {
    node.children
        .borrow()
        .iter()
        .find(|child| {
            matches!(&child.data, NodeData::Element { attrs, .. }
                if attrs.borrow().iter().any(|a| &*a.name.local == "class" && a.value.split_whitespace().any(|c| c == class)))
        })
        .cloned()
}

fn pre_handler(handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let wraps_code = {
        let children = element.node.children.borrow();
        children.len() == 1
            && matches!(&children[0].data, NodeData::Element { name, .. } if &*name.local == "code")
    };
    if wraps_code {
        return handlers.fallback(element);
    }
    let mut text = String::new();
    collect_text(element.node, &mut text);
    let text = text.trim_matches('\n');
    let language = brush_language(&element).unwrap_or_default();
    Some(fenced(text, &language).into())
}

/// `text` in a fence longer than any backtick run inside it.
fn fenced(text: &str, language: &str) -> String {
    let fence = "`".repeat(longest_backtick_run(text).max(2) + 1);
    format!("\n\n{fence}{language}\n{text}\n{fence}\n\n")
}

fn collect_text(node: &Node, out: &mut String) {
    if let NodeData::Text { contents } = &node.data {
        out.push_str(&contents.borrow());
    }
    for child in node.children.borrow().iter() {
        collect_text(child, out);
    }
}

/// `bash` from `data-syntaxhighlighter-params="brush: bash; gutter: false"`.
fn brush_language(element: &Element) -> Option<String> {
    let params = element
        .attrs
        .iter()
        .find(|attr| &*attr.name.local == "data-syntaxhighlighter-params")?;
    params
        .value
        .split(';')
        .find_map(|part| part.trim().strip_prefix("brush:"))
        .map(|language| language.trim().to_string())
        .filter(|language| !language.is_empty())
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/markdown_tests.rs"]
mod tests;
