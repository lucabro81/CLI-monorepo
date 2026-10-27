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
    let markdown = HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style"])
        .add_handler(vec!["pre"], pre_handler)
        .add_handler(vec!["div"], panel_handler)
        .add_handler(vec!["img"], image_handler)
        .add_handler(vec!["td", "th"], cell_handler)
        .build()
        .convert(html)
        // html5ever reads from the in-memory string: its I/O error cannot happen.
        .map_err(|e| CliError::Internal(format!("converting the page body to Markdown failed: {e}")))?
        .replace(CELL_PIPE, "\\|");
    Ok(match site_url {
        Some(site_url) => markdown.replace("](/wiki/", &format!("]({}/wiki/", site_url.trim_end_matches('/'))),
        None => markdown,
    })
}

/// Panel macros and the label their blockquote starts with.
const PANELS: [(&str, &str); 6] =
    [("info", "Info"), ("note", "Note"), ("warning", "Warning"), ("tip", "Tip"), ("success", "Success"), ("error", "Error")];

fn panel_handler(handlers: &dyn Handlers, element: Element) -> Option<HandlerResult> {
    let label = attr(&element, "data-macro-name")
        .and_then(|name| PANELS.iter().find(|(macro_name, _)| *macro_name == name))
        .map(|(_, label)| *label);
    let Some(label) = label else {
        return handlers.fallback(element);
    };
    // The panel's text, without its icon (an empty <span>, outside the body div).
    let body = find_child_with_class(element.node, "confluence-information-macro-body")
        .unwrap_or_else(|| Rc::clone(element.node));
    let content = handlers.walk_children(&body).content;
    let quoted: Vec<String> = format!("**{label}:** {}", content.trim())
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
    let fence = "`".repeat(longest_backtick_run(text).max(2) + 1);
    Some(format!("\n\n{fence}{language}\n{text}\n{fence}\n\n").into())
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
