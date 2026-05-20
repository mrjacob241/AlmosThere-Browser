use crate::{JsError, Program, parse_script};

#[derive(Clone, Debug, PartialEq)]
pub struct InlineScript {
    pub index: usize,
    pub source: String,
    pub program: Result<Program, JsError>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptParseReport {
    pub scripts: Vec<InlineScript>,
}

impl ScriptParseReport {
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.scripts
            .iter()
            .filter(|script| script.program.is_err())
            .count()
    }
}

pub fn parse_inline_scripts_from_html(html: &str) -> ScriptParseReport {
    let mut scripts = Vec::new();
    let mut remaining = html;
    let mut index = 0;

    while let Some(open_index) = find_ascii_case_insensitive(remaining, "<script") {
        let after_open = &remaining[open_index..];
        let Some(open_end) = after_open.find('>') else {
            break;
        };
        let content_start = open_index + open_end + 1;
        let after_content_start = &remaining[content_start..];
        let Some(close_index) = find_ascii_case_insensitive(after_content_start, "</script>")
        else {
            break;
        };

        let source = after_content_start[..close_index].to_owned();
        scripts.push(InlineScript {
            index,
            program: parse_script(&source),
            source,
        });
        index += 1;
        remaining = &after_content_start[close_index + "</script>".len()..];
    }

    ScriptParseReport { scripts }
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_multiple_inline_scripts_in_order() {
        let report = parse_inline_scripts_from_html(
            r#"<script>window.value = "A";</script><p>x</p><script>window.value = window.value + "B";</script>"#,
        );
        assert_eq!(report.scripts.len(), 2);
        assert_eq!(report.error_count(), 0);
        assert!(report.scripts[0].source.contains("\"A\""));
        assert!(report.scripts[1].source.contains("\"B\""));
    }
}
