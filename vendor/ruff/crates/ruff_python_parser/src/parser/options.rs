use ruff_python_ast::{PySourceType, PythonVersion};

use crate::{AsMode, Mode};

/// Options for controlling how a source file is parsed.
///
/// You can construct a [`ParseOptions`] directly from a [`Mode`]:
///
/// ```
/// use ruff_python_parser::{Mode, ParseOptions};
///
/// let options = ParseOptions::from(Mode::Module);
/// ```
///
/// or from a [`PySourceType`]
///
/// ```
/// use ruff_python_ast::PySourceType;
/// use ruff_python_parser::ParseOptions;
///
/// let options = ParseOptions::from(PySourceType::Python);
/// ```
#[derive(Clone, Debug)]
pub struct ParseOptions {
    /// Specify the mode in which the code will be parsed.
    pub(crate) mode: Mode,
    /// Target version for detecting version-related syntax errors.
    pub(crate) target_version: PythonVersion,
    /// Omit only duplicate keyword argument validation; retain all syntax diagnostics.
    pub(crate) ast_only: bool,
}

impl ParseOptions {
    /// Omit duplicate call and class-header keyword validation when enabled.
    ///
    /// Defaults to false. This is not a general AST-versus-compiler semantic mode.
    /// Both ordinary and unsupported-version errors must still be checked before AST use.
    #[must_use]
    pub fn with_ast_only(mut self, ast_only: bool) -> Self {
        self.ast_only = ast_only;
        self
    }

    #[must_use]
    pub fn with_target_version(mut self, target_version: PythonVersion) -> Self {
        self.target_version = target_version;
        self
    }

    pub fn target_version(&self) -> PythonVersion {
        self.target_version
    }
}

impl From<Mode> for ParseOptions {
    fn from(mode: Mode) -> Self {
        Self {
            mode,
            target_version: PythonVersion::default(),
            ast_only: false,
        }
    }
}

impl From<PySourceType> for ParseOptions {
    fn from(source_type: PySourceType) -> Self {
        Self {
            mode: source_type.as_mode(),
            target_version: PythonVersion::default(),
            ast_only: false,
        }
    }
}
