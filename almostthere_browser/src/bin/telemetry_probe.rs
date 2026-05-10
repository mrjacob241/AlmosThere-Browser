use std::{collections::BTreeMap, env, fs, io, path::Path};

fn main() -> io::Result<()> {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: telemetry_probe <session.jsonl> [event-name]");
        std::process::exit(2);
    };
    let event_filter = args.next();
    let events = read_events(Path::new(&path))?;

    if let Some(event_filter) = event_filter {
        for event in events.iter().filter(|event| event.event == event_filter) {
            println!("{}", event.raw);
        }
        return Ok(());
    }

    let summary = summarize_events(&events);
    println!("events: {}", events.len());
    println!("event_counts:");
    for (event, count) in summary.event_counts {
        println!("  {event}: {count}");
    }
    if let Some(last) = events.last() {
        println!("last_event: {}", last.event);
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TelemetryEvent {
    event: String,
    raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TelemetrySummary {
    event_counts: BTreeMap<String, usize>,
}

fn read_events(path: &Path) -> io::Result<Vec<TelemetryEvent>> {
    let content = fs::read_to_string(path)?;
    Ok(parse_events(&content))
}

fn parse_events(content: &str) -> Vec<TelemetryEvent> {
    content
        .lines()
        .filter_map(|line| {
            let event = json_string_field(line, "event")?;
            Some(TelemetryEvent {
                event,
                raw: line.to_owned(),
            })
        })
        .collect()
}

fn summarize_events(events: &[TelemetryEvent]) -> TelemetrySummary {
    let mut event_counts = BTreeMap::new();
    for event in events {
        *event_counts.entry(event.event.clone()).or_insert(0) += 1;
    }
    TelemetrySummary { event_counts }
}

fn json_string_field(line: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let mut out = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_summarizes_jsonl_events() {
        let events = parse_events(r#"{"event":"session.started"}"#.to_owned().as_str());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "session.started");

        let events = parse_events(
            r#"{"event":"session.started"}
{"event":"input.changed"}
{"event":"input.changed"}"#,
        );
        let summary = summarize_events(&events);
        assert_eq!(summary.event_counts["session.started"], 1);
        assert_eq!(summary.event_counts["input.changed"], 2);
    }

    #[test]
    fn parses_escaped_json_string_field() {
        assert_eq!(
            json_string_field(
                r#"{"event":"js.error","message":"bad \"quote\""}"#,
                "message"
            ),
            Some("bad \"quote\"".to_owned())
        );
    }
}
