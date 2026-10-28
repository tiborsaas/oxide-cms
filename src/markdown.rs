use pulldown_cmark::{html, Options, Parser};

/// Converts markdown text to HTML.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html_basic() {
        let md = "# Title\n\nThis is **bold** and *italic*.\n\n[Link](https://example.com)";
        let html = markdown_to_html(md);
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains(r#"<a href="https://example.com">Link</a>"#));
    }

    #[test]
    fn test_markdown_to_html_code_and_lists() {
        let md = "```rust\nlet x = 42;\n```\n\n- item 1\n- item 2";
        let html = markdown_to_html(md);
        assert!(html.contains(r#"<code class="language-rust">let x = 42;"#));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>item 1</li>"));
    }

    #[test]
    fn test_markdown_to_html_tables_and_strikethrough() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n\n~~deleted~~";
        let html = markdown_to_html(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>deleted</del>"));
    }
}
