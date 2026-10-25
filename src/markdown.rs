use std::collections::HashMap;
use pulldown_cmark::{html, Options, Parser};
use tera::{try_get_value, Error, Value};

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

/// Tera filter function for converting markdown to HTML.
pub fn markdown_filter(
    value: &Value,
    _args: &HashMap<String, Value>,
) -> Result<Value, Error> {
    let s = try_get_value!("markdown", "value", String, value);
    let html = markdown_to_html(&s);
    Ok(Value::String(html))
}

/// Creates a new Tera instance with templates and registers all custom filters (including markdown).
pub fn init_tera() -> Result<tera::Tera, tera::Error> {
    let mut tera = tera::Tera::new("templates/**/*.html")?;
    register_custom_filters(&mut tera);
    Ok(tera)
}

/// Registers custom filters on an existing Tera instance.
pub fn register_custom_filters(tera: &mut tera::Tera) {
    tera.register_filter("markdown", markdown_filter);
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

    #[test]
    fn test_tera_markdown_filter() {
        let mut tera = tera::Tera::default();
        register_custom_filters(&mut tera);
        tera.add_raw_template("test.html", "{{ content | markdown | safe }}")
            .unwrap();

        let mut ctx = tera::Context::new();
        ctx.insert("content", "# Hello World\n\nWelcome to **Oxide**!");

        let rendered = tera.render("test.html", &ctx).unwrap();
        assert!(rendered.contains("<h1>Hello World</h1>"));
        assert!(rendered.contains("Welcome to <strong>Oxide</strong>!"));
    }
}
