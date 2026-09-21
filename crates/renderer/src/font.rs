//! Renderer-owned font discovery and initial face selection.
//!
//! Shaping, fallback execution, rasterization, and glyph caching intentionally
//! build on this retained database later.

use std::error::Error;
use std::fmt;

use fontdb::{Database, Family, ID, Query};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum FontRequest {
    /// Use the platform's configured monospace family.
    #[default]
    SystemMonospace,
    /// Require this named monospace family.
    Family(String),
}

#[derive(Debug)]
pub(super) enum FontLoadError {
    NoSystemMonospaceFace,
    RequestedFamilyUnavailable(String),
    RequestedFamilyIsNotMonospace(String),
}

impl fmt::Display for FontLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSystemMonospaceFace => {
                formatter.write_str("no usable system monospace font face was found")
            }
            Self::RequestedFamilyUnavailable(family) => {
                write!(formatter, "requested font family `{family}` was not found")
            }
            Self::RequestedFamilyIsNotMonospace(family) => {
                write!(
                    formatter,
                    "requested font family `{family}` is not monospaced"
                )
            }
        }
    }
}

impl Error for FontLoadError {}

/// Loaded font data retained by the renderer for future shaping and rasterization.
#[derive(Debug)]
pub(super) struct FontSystem {
    database: Database,
    primary_face: ID,
}

impl FontSystem {
    /// Discovers platform fonts and selects an initial terminal face.
    pub(super) fn load_system(request: FontRequest) -> Result<Self, FontLoadError> {
        let mut database = Database::new();
        database.load_system_fonts();
        Self::from_database(database, request)
    }

    fn from_database(database: Database, request: FontRequest) -> Result<Self, FontLoadError> {
        let primary_face = select_primary_face(&database, &request)?;
        Ok(Self {
            database,
            primary_face,
        })
    }

    /// Gives a later shaping or rasterization stage temporary access to the selected face bytes.
    #[allow(dead_code)]
    pub(super) fn with_primary_face_data<T>(
        &self,
        operation: impl FnOnce(&[u8], u32) -> T,
    ) -> Option<T> {
        self.database.with_face_data(self.primary_face, operation)
    }
}

fn select_primary_face(database: &Database, request: &FontRequest) -> Result<ID, FontLoadError> {
    let (family, missing_error) = match request {
        FontRequest::SystemMonospace => (Family::Monospace, FontLoadError::NoSystemMonospaceFace),
        FontRequest::Family(name) => (
            Family::Name(name),
            FontLoadError::RequestedFamilyUnavailable(name.clone()),
        ),
    };
    let id = database.query(&Query {
        families: &[family],
        ..Default::default()
    });
    let Some(id) = id else {
        return Err(missing_error);
    };
    if database.face(id).is_some_and(|face| face.monospaced) {
        Ok(id)
    } else {
        match request {
            FontRequest::SystemMonospace => Err(FontLoadError::NoSystemMonospaceFace),
            FontRequest::Family(name) => {
                Err(FontLoadError::RequestedFamilyIsNotMonospace(name.clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontdb::{FaceInfo, Language, Source, Stretch, Style, Weight};

    use super::{FontLoadError, FontRequest, FontSystem};

    fn database_with_face(family: &str, monospaced: bool) -> fontdb::Database {
        let mut database = fontdb::Database::new();
        database.push_face_info(FaceInfo {
            id: fontdb::ID::dummy(),
            source: Source::Binary(Arc::new(vec![1, 2, 3])),
            index: 0,
            families: vec![(family.to_owned(), Language::English_UnitedStates)],
            post_script_name: format!("{family}-Regular"),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced,
        });
        database
    }

    #[test]
    fn default_request_selects_the_configured_monospace_family() {
        let mut database = database_with_face("Test Mono", true);
        database.set_monospace_family("Test Mono");

        let system = FontSystem::from_database(database, FontRequest::default()).unwrap();

        assert_eq!(
            system.with_primary_face_data(|data, index| (data.to_vec(), index)),
            Some((vec![1, 2, 3], 0))
        );
    }

    #[test]
    fn default_request_reports_when_no_system_monospace_face_is_available() {
        let error =
            FontSystem::from_database(fontdb::Database::new(), FontRequest::default()).unwrap_err();

        assert!(matches!(error, FontLoadError::NoSystemMonospaceFace));
    }

    #[test]
    fn named_request_requires_a_matching_monospace_face() {
        let database = database_with_face("Proportional", false);

        let error =
            FontSystem::from_database(database, FontRequest::Family("Proportional".to_owned()))
                .unwrap_err();

        assert!(matches!(
            error,
            FontLoadError::RequestedFamilyIsNotMonospace(ref family) if family == "Proportional"
        ));
    }

    #[test]
    fn named_request_reports_a_missing_family() {
        let database = database_with_face("Available Mono", true);

        let error = FontSystem::from_database(database, FontRequest::Family("Missing".to_owned()))
            .unwrap_err();

        assert!(matches!(
            error,
            FontLoadError::RequestedFamilyUnavailable(ref family) if family == "Missing"
        ));
    }
}
