//! HTML → Markdown conversion for page bodies (issue #254), on top of `htmd`.
//!
//! Two adjustments to htmd's defaults, both for Confluence's rendered HTML:
//! - `<script>`/`<style>` are dropped (htmd would print their text).
//! - A `<pre>` without a `<code>` child (Confluence's code and noformat macros
//!   render that way) becomes a fenced block of its raw text, with the code
//!   macro's language (`data-syntaxhighlighter-params="brush: bash; ..."`).
//!   htmd would print it as plain, Markdown-escaped text.

use htmd::element_handler::{HandlerResult, Handlers};
use htmd::{Element, HtmlToMarkdown, Node};
use markup5ever_rcdom::NodeData;

use crate::error::CliError;

pub fn html_to_markdown(html: &str) -> Result<String, CliError> {
    HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style"])
        .add_handler(vec!["pre"], pre_handler)
        .build()
        .convert(html)
        // html5ever reads from the in-memory string: its I/O error cannot happen.
        .map_err(|e| CliError::Internal(format!("converting the page body to Markdown failed: {e}")))
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
