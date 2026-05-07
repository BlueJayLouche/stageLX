use thiserror::Error;

#[derive(Debug, Error)]
pub enum GdtfError {
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("xml deserialization error: {0}")]
    XmlParse(#[from] quick_xml::de::DeError),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
