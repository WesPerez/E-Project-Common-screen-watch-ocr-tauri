#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFlavor {
    Lite,
    Full,
}

pub const BUILD_FLAVOR_ENV: &str = "SCREENWATCH_BUILD_FLAVOR";
pub const COMPILED_BUILD_FLAVOR_ENV: &str = "SCREENWATCH_COMPILED_BUILD_FLAVOR";

impl BuildFlavor {
    pub fn from_env() -> Self {
        Self::compiled()
    }

    pub fn compiled() -> Self {
        Self::from_compiled_parts(
            option_env!("SCREENWATCH_COMPILED_BUILD_FLAVOR"),
            cfg!(feature = "ocr"),
        )
    }

    pub fn from_value(value: Option<&str>) -> Self {
        match value.map(|item| item.trim().to_ascii_lowercase()) {
            Some(value) if value == "full" => Self::Full,
            _ => Self::Lite,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Full => "full",
        }
    }

    pub fn ocr_enabled(self) -> bool {
        matches!(self, Self::Full)
    }

    fn from_compiled_parts(value: Option<&str>, ocr_feature_compiled: bool) -> Self {
        value
            .map(|value| Self::from_value(Some(value)))
            .unwrap_or_else(|| {
                if ocr_feature_compiled {
                    Self::Full
                } else {
                    Self::Lite
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{BuildFlavor, BUILD_FLAVOR_ENV, COMPILED_BUILD_FLAVOR_ENV};

    #[test]
    fn build_flavor_defaults_to_lite() {
        assert_eq!(BuildFlavor::from_value(None), BuildFlavor::Lite);
        assert_eq!(BuildFlavor::from_value(Some("")), BuildFlavor::Lite);
        assert_eq!(BuildFlavor::from_value(Some("lite")), BuildFlavor::Lite);
    }

    #[test]
    fn build_flavor_accepts_full_case_insensitively() {
        assert_eq!(BuildFlavor::from_value(Some("FULL")), BuildFlavor::Full);
        assert!(BuildFlavor::Full.ocr_enabled());
        assert!(!BuildFlavor::Lite.ocr_enabled());
    }

    #[test]
    fn compiled_build_flavor_falls_back_to_ocr_feature_boundary() {
        assert_eq!(
            BuildFlavor::from_compiled_parts(None, false),
            BuildFlavor::Lite
        );
        assert_eq!(
            BuildFlavor::from_compiled_parts(None, true),
            BuildFlavor::Full
        );
        assert_eq!(
            BuildFlavor::from_compiled_parts(Some("lite"), true),
            BuildFlavor::Lite
        );
        assert_eq!(
            BuildFlavor::from_compiled_parts(Some("full"), false),
            BuildFlavor::Full
        );
        if option_env!("SCREENWATCH_COMPILED_BUILD_FLAVOR").is_none() {
            assert_eq!(BuildFlavor::compiled().ocr_enabled(), cfg!(feature = "ocr"));
        }
    }

    #[test]
    fn packaged_build_flavor_is_compile_time_state() {
        assert_eq!(BUILD_FLAVOR_ENV, "SCREENWATCH_BUILD_FLAVOR");
        assert_eq!(
            COMPILED_BUILD_FLAVOR_ENV,
            "SCREENWATCH_COMPILED_BUILD_FLAVOR"
        );
        assert!(matches!(
            option_env!("SCREENWATCH_COMPILED_BUILD_FLAVOR"),
            Some("lite" | "full")
        ));
        assert_eq!(BuildFlavor::from_env(), BuildFlavor::compiled());
    }
}
