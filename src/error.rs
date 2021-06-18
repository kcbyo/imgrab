use std::{
    error,
    fmt::{self, Display},
    io,
};

use crate::config::Key;

#[derive(Debug)]
pub enum Error {
    Configuration(Key),
    Extraction(ExtractionFailure, String),
    Io(io::Error),
    Network(reqwest::Error),
    Unsupported(UnsupportedError, String),
    Url(url::ParseError),

    // This is a catch-all for bullshit like int parsing errors.
    Other(String, Box<(dyn error::Error + 'static)>),
}

#[derive(Copy, Clone, Debug)]
pub enum ExtractionFailure {
    Metadata,
    ImageUrl,
}

#[derive(Copy, Clone, Debug)]
pub enum UnsupportedError {
    Domain,
    Route,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Configuration(key) => write!(f, "Configuration not provided: {:?}", key),
            Error::Extraction(kind, url) => match kind {
                ExtractionFailure::Metadata => {
                    write!(f, "Unable to extract gallery metadata at {}", url)
                }
                ExtractionFailure::ImageUrl => write!(f, "Unable to extract image url at {}", url),
            },

            Error::Io(_) => f.write_str("IO error"),
            Error::Network(_) => f.write_str("Network error"),
            Error::Unsupported(UnsupportedError::Domain, url) => {
                write!(f, "Unsupported domain: {}", url)
            }
            Error::Unsupported(UnsupportedError::Route, url) => {
                write!(f, "Unsupported object type: {}", url)
            }
            Error::Url(_) => f.write_str("Bad url"),

            Error::Other(message, _) => f.write_str(&message),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Configuration(_) => None,
            Error::Extraction(..) => None,
            Error::Io(e) => Some(e),
            Error::Network(e) => Some(e),
            Error::Unsupported(..) => None,
            Error::Url(e) => Some(e),

            Error::Other(_, e) => Some(e.as_ref()),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Network(e)
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error::Url(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
