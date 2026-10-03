use aether_crds::v1alpha::identity_instance::{Branding, BrandingColors, IamPhase, IamStatus};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeDecision<'a> {
    Apply(&'a Branding),
    RestoreDefault,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    Retry { message: String },
    Rejected { message: String },
}

fn colors_are_empty(colors: &BrandingColors) -> bool {
    [
        &colors.primary,
        &colors.primary_text,
        &colors.links,
        &colors.page_background,
        &colors.widget_background,
        &colors.text,
        &colors.error,
    ]
    .iter()
    .all(|color| color.is_none())
}

fn is_empty(branding: &Branding) -> bool {
    branding.radius.is_none() && branding.colors.as_ref().is_none_or(colors_are_empty)
}

pub fn marker(branding: &Branding) -> String {
    serde_json::to_string(branding).unwrap_or_default()
}

pub fn decide<'a>(branding: Option<&'a Branding>, applied: Option<&str>) -> ThemeDecision<'a> {
    match (branding.filter(|branding| !is_empty(branding)), applied) {
        (Some(branding), Some(applied)) if marker(branding) == applied => ThemeDecision::Nothing,
        (Some(branding), _) => ThemeDecision::Apply(branding),
        (None, Some(_)) => ThemeDecision::RestoreDefault,
        (None, None) => ThemeDecision::Nothing,
    }
}

pub fn default_theme_config() -> Value {
    json!({})
}

pub fn theme_config(branding: &Branding) -> Value {
    let mut config = Map::new();

    if let Some(colors) = branding.colors.as_ref() {
        let mut mapped = Map::new();
        let pairs = [
            ("primaryButton", &colors.primary),
            ("primaryButtonLabel", &colors.primary_text),
            ("links", &colors.links),
            ("pageBackground", &colors.page_background),
            ("widgetBackground", &colors.widget_background),
            ("bodyText", &colors.text),
            ("error", &colors.error),
        ];
        for (key, color) in pairs {
            if let Some(color) = color {
                mapped.insert(key.to_string(), json!(color));
            }
        }
        if !mapped.is_empty() {
            config.insert("colors".to_string(), Value::Object(mapped));
        }
    }

    if let Some(radius) = branding.radius {
        config.insert(
            "borders".to_string(),
            json!({
                "widgetRadius": radius,
                "buttonRadius": radius,
                "inputRadius": radius,
            }),
        );
    }

    Value::Object(config)
}

pub fn outcome_status(
    previous: Option<&IamStatus>,
    target: Option<String>,
    outcome: Result<(), ThemeError>,
    observed_at: String,
) -> IamStatus {
    let kept = previous.and_then(|status| status.applied.clone());
    let (phase, message, applied) = match outcome {
        Ok(()) => (IamPhase::Applied, None, target),
        Err(ThemeError::Retry { message }) => (IamPhase::Pending, Some(message), kept),
        Err(ThemeError::Rejected { message }) => (IamPhase::Failed, Some(message), kept),
    };
    IamStatus {
        phase,
        message,
        applied,
        observed_at: Some(observed_at),
    }
}

pub fn differs(previous: Option<&IamStatus>, next: &IamStatus) -> bool {
    previous.is_none_or(|previous| {
        previous.phase != next.phase
            || previous.message != next.message
            || previous.applied != next.applied
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> BrandingColors {
        BrandingColors {
            primary: None,
            primary_text: None,
            links: None,
            page_background: None,
            widget_background: None,
            text: None,
            error: None,
        }
    }

    fn branding(colors: Option<BrandingColors>, radius: Option<u8>) -> Branding {
        Branding { colors, radius }
    }

    #[test]
    fn each_color_maps_to_its_theme_key() {
        let cases: [(BrandingColors, &str); 7] = [
            (
                BrandingColors {
                    primary: Some("#111111".into()),
                    ..colors()
                },
                "primaryButton",
            ),
            (
                BrandingColors {
                    primary_text: Some("#111111".into()),
                    ..colors()
                },
                "primaryButtonLabel",
            ),
            (
                BrandingColors {
                    links: Some("#111111".into()),
                    ..colors()
                },
                "links",
            ),
            (
                BrandingColors {
                    page_background: Some("#111111".into()),
                    ..colors()
                },
                "pageBackground",
            ),
            (
                BrandingColors {
                    widget_background: Some("#111111".into()),
                    ..colors()
                },
                "widgetBackground",
            ),
            (
                BrandingColors {
                    text: Some("#111111".into()),
                    ..colors()
                },
                "bodyText",
            ),
            (
                BrandingColors {
                    error: Some("#111111".into()),
                    ..colors()
                },
                "error",
            ),
        ];
        for (colors, key) in cases {
            let config = theme_config(&branding(Some(colors), None));
            assert_eq!(config, json!({"colors": {key: "#111111"}}));
        }
    }

    #[test]
    fn only_some_fields_leave_the_rest_out() {
        let config = theme_config(&branding(
            Some(BrandingColors {
                primary: Some("#aabbcc".into()),
                error: Some("#ff0000".into()),
                ..colors()
            }),
            None,
        ));
        assert_eq!(
            config,
            json!({"colors": {"primaryButton": "#aabbcc", "error": "#ff0000"}})
        );
    }

    #[test]
    fn nothing_set_is_an_empty_config() {
        assert_eq!(theme_config(&branding(None, None)), json!({}));
        assert_eq!(theme_config(&branding(Some(colors()), None)), json!({}));
    }

    #[test]
    fn radius_goes_to_three_keys() {
        assert_eq!(
            theme_config(&branding(None, Some(8))),
            json!({"borders": {"widgetRadius": 8, "buttonRadius": 8, "inputRadius": 8}})
        );
    }

    #[test]
    fn zero_radius_is_still_a_radius() {
        let config = theme_config(&branding(None, Some(0)));
        assert_eq!(config["borders"]["widgetRadius"], json!(0));
    }

    #[test]
    fn decision_table() {
        let set = branding(None, Some(6));
        let other = branding(None, Some(7));
        let empty = branding(Some(colors()), None);
        let applied = marker(&set);

        assert_eq!(decide(Some(&set), None), ThemeDecision::Apply(&set));
        assert_eq!(
            decide(Some(&other), Some(&applied)),
            ThemeDecision::Apply(&other)
        );
        assert_eq!(decide(Some(&set), Some(&applied)), ThemeDecision::Nothing);
        assert_eq!(decide(None, Some(&applied)), ThemeDecision::RestoreDefault);
        assert_eq!(
            decide(Some(&empty), Some(&applied)),
            ThemeDecision::RestoreDefault
        );
        assert_eq!(decide(None, None), ThemeDecision::Nothing);
        assert_eq!(decide(Some(&empty), None), ThemeDecision::Nothing);
    }

    #[test]
    fn success_records_the_marker_and_clears_the_message() {
        let previous = IamStatus {
            phase: IamPhase::Pending,
            message: Some("later".into()),
            applied: None,
            observed_at: None,
        };
        let status = outcome_status(Some(&previous), Some("m".into()), Ok(()), "t".into());
        assert_eq!(status.phase, IamPhase::Applied);
        assert_eq!(status.message, None);
        assert_eq!(status.applied.as_deref(), Some("m"));
    }

    #[test]
    fn restoring_the_default_clears_the_marker() {
        let previous = IamStatus {
            phase: IamPhase::Applied,
            message: None,
            applied: Some("m".into()),
            observed_at: None,
        };
        let status = outcome_status(Some(&previous), None, Ok(()), "t".into());
        assert_eq!(status.phase, IamPhase::Applied);
        assert_eq!(status.applied, None);
    }

    #[test]
    fn retry_is_pending_and_keeps_what_is_live() {
        let previous = IamStatus {
            phase: IamPhase::Applied,
            message: None,
            applied: Some("old".into()),
            observed_at: None,
        };
        let status = outcome_status(
            Some(&previous),
            Some("new".into()),
            Err(ThemeError::Retry {
                message: "not ready".into(),
            }),
            "t".into(),
        );
        assert_eq!(status.phase, IamPhase::Pending);
        assert_eq!(status.message.as_deref(), Some("not ready"));
        assert_eq!(status.applied.as_deref(), Some("old"));
    }

    #[test]
    fn rejection_is_failed_with_the_message() {
        let status = outcome_status(
            None,
            Some("new".into()),
            Err(ThemeError::Rejected {
                message: "bad color".into(),
            }),
            "t".into(),
        );
        assert_eq!(status.phase, IamPhase::Failed);
        assert_eq!(status.message.as_deref(), Some("bad color"));
        assert_eq!(status.applied, None);
    }

    #[test]
    fn a_new_observation_time_alone_is_not_a_change() {
        let a = outcome_status(None, None, Ok(()), "t1".into());
        let b = outcome_status(Some(&a), None, Ok(()), "t2".into());
        assert!(!differs(Some(&a), &b));
        assert!(differs(None, &b));
    }
}
