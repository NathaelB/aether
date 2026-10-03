use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidBranding {
    #[error("{field} must be a hex color like #1a2b3c, got '{value}'")]
    Color { field: &'static str, value: String },

    #[error(
        "radius must be an integer from {} to {}, got {value}",
        Radius::MIN,
        Radius::MAX
    )]
    Radius { value: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(transparent)]
pub struct HexColor(String);

impl HexColor {
    fn parse(field: &'static str, value: String) -> Result<Self, InvalidBranding> {
        let valid = value.len() == 7
            && value.starts_with('#')
            && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit());

        if valid {
            Ok(Self(value))
        } else {
            Err(InvalidBranding::Color { field, value })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(transparent)]
pub struct Radius(u8);

impl Radius {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 24;

    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<i64> for Radius {
    type Error = InvalidBranding;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        u8::try_from(value)
            .ok()
            .filter(|radius| (Self::MIN..=Self::MAX).contains(radius))
            .map(Self)
            .ok_or(InvalidBranding::Radius { value })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct BrandingColors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_text: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_background: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_background: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<HexColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HexColor>,
}

impl BrandingColors {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "BrandingInput")]
pub struct Branding {
    #[serde(skip_serializing_if = "BrandingColors::is_empty")]
    pub colors: BrandingColors,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<Radius>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrandingColorsInput {
    pub primary: Option<String>,
    pub primary_text: Option<String>,
    pub links: Option<String>,
    pub page_background: Option<String>,
    pub widget_background: Option<String>,
    pub text: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrandingInput {
    #[serde(default)]
    pub colors: BrandingColorsInput,
    pub radius: Option<i64>,
}

fn color(field: &'static str, raw: Option<String>) -> Result<Option<HexColor>, InvalidBranding> {
    raw.map(|value| HexColor::parse(field, value)).transpose()
}

impl TryFrom<BrandingColorsInput> for BrandingColors {
    type Error = InvalidBranding;

    fn try_from(input: BrandingColorsInput) -> Result<Self, Self::Error> {
        Ok(Self {
            primary: color("primary", input.primary)?,
            primary_text: color("primary_text", input.primary_text)?,
            links: color("links", input.links)?,
            page_background: color("page_background", input.page_background)?,
            widget_background: color("widget_background", input.widget_background)?,
            text: color("text", input.text)?,
            error: color("error", input.error)?,
        })
    }
}

impl TryFrom<BrandingInput> for Branding {
    type Error = InvalidBranding;

    fn try_from(input: BrandingInput) -> Result<Self, Self::Error> {
        Ok(Self {
            colors: input.colors.try_into()?,
            radius: input.radius.map(Radius::try_from).transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> Result<Branding, serde_json::Error> {
        serde_json::from_value(value)
    }

    fn refusal(input: BrandingInput) -> InvalidBranding {
        Branding::try_from(input).expect_err("refused")
    }

    fn with_primary(value: &str) -> BrandingInput {
        BrandingInput {
            colors: BrandingColorsInput {
                primary: Some(value.to_string()),
                ..Default::default()
            },
            radius: None,
        }
    }

    #[test]
    fn a_full_branding_reads_and_writes_back_unchanged() {
        let wire = json!({
            "colors": {
                "primary": "#112233",
                "primary_text": "#FFFFFF",
                "links": "#0a0b0c",
                "page_background": "#000000",
                "widget_background": "#eeeeee",
                "text": "#333333",
                "error": "#ff0000"
            },
            "radius": 6
        });

        let branding = parse(wire.clone()).expect("valid");

        assert_eq!(serde_json::to_value(&branding).unwrap(), wire);
    }

    #[test]
    fn every_part_is_optional() {
        assert_eq!(parse(json!({})).expect("empty"), Branding::default());
        assert_eq!(
            serde_json::to_value(Branding::default()).unwrap(),
            json!({})
        );
        assert!(parse(json!({ "radius": 0 })).is_ok());
        assert!(parse(json!({ "colors": { "error": "#abcdef" } })).is_ok());
    }

    #[test]
    fn a_color_is_exactly_a_hash_and_six_hex_digits() {
        for bad in [
            "",
            "#",
            "112233",
            "#12345",
            "#1234567",
            "#12345g",
            "red",
            "#ggg000",
            " #112233",
            "#11223 ",
            "#１１２２３３",
            "rgb(0,0,0)",
            "#112233ff",
        ] {
            assert!(
                matches!(
                    refusal(with_primary(bad)),
                    InvalidBranding::Color {
                        field: "primary",
                        ..
                    }
                ),
                "{bad:?} should be refused"
            );
        }
    }

    #[test]
    fn a_color_is_accepted_in_either_case() {
        for good in ["#aabbcc", "#AABBCC", "#aAbBcC", "#012345"] {
            let branding = Branding::try_from(with_primary(good)).expect(good);
            assert_eq!(branding.colors.primary.unwrap().as_str(), good);
        }
    }

    #[test]
    fn the_refusal_names_the_color_that_is_wrong() {
        let input = BrandingInput {
            colors: BrandingColorsInput {
                primary: Some("#112233".into()),
                page_background: Some("blue".into()),
                ..Default::default()
            },
            radius: None,
        };

        let message = refusal(input).to_string();

        assert!(message.contains("page_background"), "{message}");
        assert!(message.contains("blue"), "{message}");
    }

    #[test]
    fn each_color_field_is_checked() {
        for name in [
            "primary",
            "primary_text",
            "links",
            "page_background",
            "widget_background",
            "text",
            "error",
        ] {
            let colors: BrandingColorsInput =
                serde_json::from_value(json!({ name: "x" })).expect("a field");

            let error = refusal(BrandingInput {
                colors,
                radius: None,
            });

            assert!(
                matches!(error, InvalidBranding::Color { field, .. } if field == name),
                "{name}"
            );
        }
    }

    #[test]
    fn a_radius_is_an_integer_from_zero_to_twenty_four() {
        for good in [0, 1, 24] {
            let branding = Branding::try_from(BrandingInput {
                colors: Default::default(),
                radius: Some(good),
            })
            .expect("in range");
            assert_eq!(i64::from(branding.radius.unwrap().get()), good);
        }

        for bad in [-1, 25, 256, 1000, i64::MIN, i64::MAX] {
            assert_eq!(
                refusal(BrandingInput {
                    colors: Default::default(),
                    radius: Some(bad),
                }),
                InvalidBranding::Radius { value: bad }
            );
        }
    }

    #[test]
    fn a_radius_that_is_not_an_integer_is_not_read() {
        assert!(parse(json!({ "radius": 6.5 })).is_err());
        assert!(parse(json!({ "radius": "6" })).is_err());
    }

    #[test]
    fn invalid_values_cannot_come_through_deserialisation() {
        assert!(parse(json!({ "radius": 25 })).is_err());
        assert!(parse(json!({ "colors": { "links": "#12" } })).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected_at_both_levels() {
        assert!(parse(json!({ "font": "serif" })).is_err());
        assert!(parse(json!({ "colors": { "accent": "#112233" } })).is_err());
        assert!(serde_json::from_value::<BrandingInput>(json!({ "font": "serif" })).is_err());
    }
}
