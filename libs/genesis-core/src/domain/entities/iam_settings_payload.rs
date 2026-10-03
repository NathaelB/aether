use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::error::GenesisError;

const MAX_RADIUS: u8 = 24;

/// Decoded payload of a `deployment.iam_settings` action, schema version 1.
///
/// `branding` is null for "none", written through as the absence of the field
/// rather than as an empty one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IamSettingsPayloadV1 {
    pub deployment_id: Uuid,
    pub namespace: String,
    #[serde(default)]
    pub branding: Option<Branding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branding {
    #[serde(default)]
    pub colors: Option<BrandingColors>,
    #[serde(default)]
    pub radius: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrandingColors {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub primary_text: Option<String>,
    #[serde(default)]
    pub links: Option<String>,
    #[serde(default)]
    pub page_background: Option<String>,
    #[serde(default)]
    pub widget_background: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl IamSettingsPayloadV1 {
    pub fn from_value(payload: &Value) -> Result<Self, GenesisError> {
        let parsed: Self =
            serde_json::from_value(payload.clone()).map_err(|e| GenesisError::InvalidPayload {
                message: e.to_string(),
            })?;

        if let Some(branding) = &parsed.branding {
            branding.validate()?;
        }

        Ok(parsed)
    }
}

impl Branding {
    fn validate(&self) -> Result<(), GenesisError> {
        if let Some(radius) = self.radius
            && radius > MAX_RADIUS
        {
            return Err(invalid(format!(
                "radius {radius} is outside 0..={MAX_RADIUS}"
            )));
        }

        if let Some(colors) = &self.colors {
            for (name, value) in colors.named() {
                if let Some(value) = value
                    && !is_hex_color(value)
                {
                    return Err(invalid(format!("{name} is not a #rrggbb color")));
                }
            }
        }

        Ok(())
    }

    /// The camelCase shape the resource carries. Every key is written, absent
    /// ones as null, because a merge patch only removes what it names.
    pub fn to_resource(&self) -> Value {
        let colors = match &self.colors {
            Some(colors) => Value::Object(
                colors
                    .resource_named()
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.into()))
                    .collect(),
            ),
            None => Value::Null,
        };

        serde_json::json!({ "colors": colors, "radius": self.radius })
    }
}

impl BrandingColors {
    fn named(&self) -> [(&'static str, &Option<String>); 7] {
        [
            ("primary", &self.primary),
            ("primary_text", &self.primary_text),
            ("links", &self.links),
            ("page_background", &self.page_background),
            ("widget_background", &self.widget_background),
            ("text", &self.text),
            ("error", &self.error),
        ]
    }

    fn resource_named(&self) -> [(&'static str, Option<&str>); 7] {
        [
            ("primary", self.primary.as_deref()),
            ("primaryText", self.primary_text.as_deref()),
            ("links", self.links.as_deref()),
            ("pageBackground", self.page_background.as_deref()),
            ("widgetBackground", self.widget_background.as_deref()),
            ("text", self.text.as_deref()),
            ("error", self.error.as_deref()),
        ]
    }
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn invalid(message: String) -> GenesisError {
    GenesisError::InvalidPayload { message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(branding: Value) -> Value {
        json!({
            "deployment_id": Uuid::from_u128(1),
            "namespace": "tenant-a",
            "branding": branding,
        })
    }

    #[test]
    fn a_full_branding_is_decoded() {
        let parsed = IamSettingsPayloadV1::from_value(&payload(json!({
            "colors": { "primary": "#0A1b2C", "primary_text": "#ffffff", "links": "#000000",
                "page_background": "#111111", "widget_background": "#222222",
                "text": "#333333", "error": "#ff0000" },
            "radius": 24
        })))
        .expect("valid");

        let branding = parsed.branding.expect("branding");
        assert_eq!(branding.radius, Some(24));
        assert_eq!(
            branding.colors.expect("colors").primary.as_deref(),
            Some("#0A1b2C")
        );
    }

    #[test]
    fn null_and_missing_branding_are_none() {
        assert_eq!(
            IamSettingsPayloadV1::from_value(&payload(json!(null)))
                .expect("valid")
                .branding,
            None
        );
        let missing = json!({ "deployment_id": Uuid::from_u128(1), "namespace": "n" });
        assert_eq!(
            IamSettingsPayloadV1::from_value(&missing)
                .expect("valid")
                .branding,
            None
        );
    }

    #[test]
    fn every_invalid_color_is_refused() {
        for bad in [
            "fff", "#fff", "#gggggg", "#1234567", "red", "", "#12345", "#ffffff ",
        ] {
            let result =
                IamSettingsPayloadV1::from_value(&payload(json!({ "colors": { "links": bad } })));
            assert!(
                matches!(result, Err(GenesisError::InvalidPayload { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn each_color_key_is_checked() {
        for key in [
            "primary",
            "primary_text",
            "links",
            "page_background",
            "widget_background",
            "text",
            "error",
        ] {
            let result =
                IamSettingsPayloadV1::from_value(&payload(json!({ "colors": { key: "nope" } })));
            assert!(result.is_err(), "{key}");
        }
    }

    #[test]
    fn a_radius_out_of_range_is_refused() {
        for bad in [json!(25), json!(255), json!(-1), json!(1000), json!(1.5)] {
            let result = IamSettingsPayloadV1::from_value(&payload(json!({ "radius": bad })));
            assert!(
                matches!(result, Err(GenesisError::InvalidPayload { .. })),
                "{bad}"
            );
        }
        assert!(IamSettingsPayloadV1::from_value(&payload(json!({ "radius": 0 }))).is_ok());
    }

    #[test]
    fn unknown_fields_are_refused() {
        for branding in [
            json!({ "radius": 4, "font": "x" }),
            json!({ "colors": { "accent": "#ffffff" } }),
        ] {
            assert!(IamSettingsPayloadV1::from_value(&payload(branding)).is_err());
        }
    }

    #[test]
    fn a_payload_missing_its_deployment_is_refused() {
        let result = IamSettingsPayloadV1::from_value(&json!({ "namespace": "n" }));

        assert!(matches!(result, Err(GenesisError::InvalidPayload { .. })));
    }

    #[test]
    fn the_resource_shape_is_camel_case_with_explicit_nulls() {
        let parsed = IamSettingsPayloadV1::from_value(&payload(json!({
            "colors": { "primary_text": "#ffffff" }
        })))
        .expect("valid");

        let shape = parsed.branding.expect("branding").to_resource();

        assert_eq!(shape["colors"]["primaryText"], json!("#ffffff"));
        assert_eq!(shape["colors"]["pageBackground"], json!(null));
        assert!(
            shape["colors"]
                .as_object()
                .expect("object")
                .contains_key("pageBackground")
        );
        assert_eq!(shape["radius"], json!(null));
    }
}
