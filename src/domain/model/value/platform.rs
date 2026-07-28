use enum_as_inner::EnumAsInner;
use enum_kinds::EnumKind;
use unic_langid::LanguageIdentifier;

const SOURCE_GATEWAY: &str = "gateway";

const SOURCE_GATEWAY_ZH: &str = "网关平台";

const SOURCE_GATEWAY_EN: &str = "Gateway";

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, EnumKind)]
#[enum_kind(SourceKind, derive(EnumAsInner))]
pub enum Platform {
    #[default]
    Gateway,
    Platform(String),
}

impl Platform {
    pub fn from_str(src: &str) -> Self {
        let src = src.to_lowercase();
        match src.as_str() {
            SOURCE_GATEWAY => Self::Gateway,
            _ => Self::Platform(src),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Gateway => SOURCE_GATEWAY,
            Self::Platform(platform) => platform.as_str(),
        }
    }

    pub fn display_name(&self, language_ident: &LanguageIdentifier) -> String {
        let language = language_ident.language;
        let language_str = language.to_string().to_uppercase();
        const EN: &str = "EN";
        const ZH: &str = "ZH";

        match self {
            &Self::Gateway => match language_str.as_str() {
                EN => SOURCE_GATEWAY_EN,
                ZH => SOURCE_GATEWAY_ZH,
                _ => SOURCE_GATEWAY,
            },
            Self::Platform(platform) => platform,
        }
        .to_string()
    }
}

impl ToString for Platform {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}
