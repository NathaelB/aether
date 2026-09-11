//! Just enough of the Prometheus text exposition format to read a counter.
//!
//! A parser rather than a dependency because the whole requirement is "sum the
//! series of one counter, optionally by one label". A full client library
//! would bring a registry, a scrape loop and a histogram model Herald has no
//! use for.

/// One series of a counter: its label set exactly as written, and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct CounterSeries<'a> {
    pub labels: &'a str,
    pub value: f64,
}

/// Every series of the counter named `name`.
///
/// An empty result means the exposition carries no such counter -- which is a
/// different thing from a counter standing at zero, and the callers depend on
/// being able to tell them apart.
pub fn counter_series<'a>(exposition: &'a str, name: &str) -> Vec<CounterSeries<'a>> {
    exposition
        .lines()
        .filter_map(|line| parse_series(line.trim(), name))
        .collect()
}

fn parse_series<'a>(line: &'a str, name: &str) -> Option<CounterSeries<'a>> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let rest = line.strip_prefix(name)?;

    let (labels, rest) = match rest.strip_prefix('{') {
        Some(inside) => {
            let end = closing_brace(inside)?;
            (&inside[..end], &inside[end + 1..])
        }
        None => {
            // Without this, `foo_total_sum` would be read as a series of
            // `foo_total`: the name matched as a prefix, not as the name.
            if !rest.starts_with([' ', '\t']) {
                return None;
            }
            ("", rest)
        }
    };

    let value = rest.split_whitespace().next()?.parse::<f64>().ok()?;

    Some(CounterSeries { labels, value })
}

/// The offset of the `}` that closes a label set, skipping any inside quotes.
fn closing_brace(labels: &str) -> Option<usize> {
    let bytes = labels.as_bytes();
    let mut index = 0;
    let mut in_quotes = false;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_quotes => index += 1,
            b'"' => in_quotes = !in_quotes,
            b'}' if !in_quotes => return Some(index),
            _ => {}
        }
        index += 1;
    }

    None
}

/// The value of one label, with the format's escapes resolved.
pub fn label(labels: &str, name: &str) -> Option<String> {
    let mut rest = labels;

    loop {
        let candidate = rest.trim_start_matches([' ', '\t', ',']);
        if candidate.is_empty() {
            return None;
        }

        let equals = candidate.find('=')?;
        let key = candidate[..equals].trim();
        let quoted = candidate[equals + 1..].strip_prefix('"')?;
        let (value, remainder) = take_quoted(quoted)?;

        if key == name {
            return Some(value);
        }

        rest = remainder;
    }
}

/// Reads a quoted label value, returning it and whatever follows the closing
/// quote.
fn take_quoted(quoted: &str) -> Option<(String, &str)> {
    let mut value = String::new();
    let mut characters = quoted.char_indices();

    while let Some((index, character)) = characters.next() {
        match character {
            '"' => return Some((value, &quoted[index + 1..])),
            '\\' => match characters.next() {
                Some((_, 'n')) => value.push('\n'),
                Some((_, escaped)) => value.push(escaped),
                None => return None,
            },
            _ => value.push(character),
        }
    }

    None
}

/// Sums a counter's series, keeping only those the filter accepts.
///
/// `None` when nothing matched, and `Some(0)` when something matched and
/// summed to zero. The distinction is the point: a counter Herald cannot find
/// and a counter standing at zero must not produce the same report.
pub fn sum_where(exposition: &str, name: &str, accept: impl Fn(&str) -> bool) -> Option<u64> {
    let matching: Vec<_> = counter_series(exposition, name)
        .into_iter()
        .filter(|series| accept(series.labels))
        .collect();

    if matching.is_empty() {
        return None;
    }

    Some(
        matching
            .iter()
            .map(|series| series.value.max(0.0) as u64)
            .sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPOSITION: &str = r#"
# HELP axum_http_requests_total requests
# TYPE axum_http_requests_total counter
axum_http_requests_total{endpoint="/api/realms/{realm_name}/protocol/openid-connect/token",method="POST",status="200"} 12
axum_http_requests_total{endpoint="/api/realms/{realm_name}/protocol/openid-connect/token/introspect",method="POST",status="200"} 3
axum_http_requests_total{endpoint="/api/health",method="GET",status="200"} 400
axum_http_requests_duration_seconds_count{endpoint="/api/health"} 999
"#;

    #[test]
    fn every_series_of_a_counter_is_read() {
        assert_eq!(
            counter_series(EXPOSITION, "axum_http_requests_total").len(),
            3
        );
    }

    /// `axum_http_requests_duration_seconds_count` starts with the same name
    /// as no counter here, but a histogram's `_count` suffix is exactly how a
    /// prefix match goes wrong in this format.
    #[test]
    fn a_name_that_is_only_a_prefix_is_not_a_match() {
        let series = counter_series(EXPOSITION, "axum_http_requests");

        assert!(series.is_empty(), "got {series:?}");
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        assert!(counter_series("# TYPE foo counter\n\n", "foo").is_empty());
    }

    #[test]
    fn a_series_with_no_labels_is_read() {
        let series = counter_series(
            "process_start_time_seconds 1.7e9",
            "process_start_time_seconds",
        );

        assert_eq!(series.len(), 1);
        assert_eq!(series[0].labels, "");
    }

    #[test]
    fn a_label_is_read_out_of_a_label_set() {
        let series = counter_series(EXPOSITION, "axum_http_requests_total");

        assert_eq!(label(series[0].labels, "method").as_deref(), Some("POST"));
        assert_eq!(
            label(series[2].labels, "endpoint").as_deref(),
            Some("/api/health")
        );
    }

    #[test]
    fn a_label_that_is_not_there_is_none() {
        let series = counter_series(EXPOSITION, "axum_http_requests_total");

        assert_eq!(label(series[0].labels, "realm"), None);
    }

    /// A `}` inside a label value must not be read as the end of the label
    /// set, which is not hypothetical here: axum reports route templates, and
    /// they are full of braces.
    #[test]
    fn a_brace_inside_a_label_value_does_not_end_the_label_set() {
        let series = counter_series(EXPOSITION, "axum_http_requests_total");

        assert_eq!(
            label(series[0].labels, "endpoint").as_deref(),
            Some("/api/realms/{realm_name}/protocol/openid-connect/token")
        );
    }

    #[test]
    fn an_escaped_quote_inside_a_label_value_is_resolved() {
        let series = counter_series(r#"foo{bar="a\"b",baz="c"} 1"#, "foo");

        assert_eq!(label(series[0].labels, "bar").as_deref(), Some(r#"a"b"#));
        assert_eq!(label(series[0].labels, "baz").as_deref(), Some("c"));
    }

    #[test]
    fn a_filtered_sum_only_counts_what_matched() {
        let total = sum_where(EXPOSITION, "axum_http_requests_total", |labels| {
            label(labels, "endpoint")
                .is_some_and(|endpoint| endpoint.ends_with("/protocol/openid-connect/token"))
        });

        assert_eq!(
            total,
            Some(12),
            "introspect must not be counted as a token event"
        );
    }

    #[test]
    fn a_sum_with_no_matching_series_is_unknown_rather_than_zero() {
        let total = sum_where(EXPOSITION, "axum_http_requests_total", |labels| {
            label(labels, "endpoint").is_some_and(|endpoint| endpoint == "/nowhere")
        });

        assert_eq!(total, None);
    }

    #[test]
    fn a_sum_of_matching_series_is_their_total() {
        let total = sum_where(EXPOSITION, "axum_http_requests_total", |_| true);

        assert_eq!(total, Some(415));
    }
}
