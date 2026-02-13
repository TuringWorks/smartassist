//! Attachment handling for messages.

use crate::error::ChannelError;
use crate::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::debug;

/// An attachment to a message.
#[derive(Debug, Clone)]
pub struct Attachment {
    /// Attachment type.
    pub attachment_type: AttachmentType,

    /// File name.
    pub filename: String,

    /// MIME type.
    pub mime_type: String,

    /// Content source.
    pub source: AttachmentSource,

    /// File size in bytes (if known).
    pub size: Option<usize>,

    /// Caption/alt text.
    pub caption: Option<String>,

    /// Whether this is a spoiler (blurred).
    pub spoiler: bool,
}

/// Type of attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    /// Image (png, jpg, gif, webp).
    Image,

    /// Audio file.
    Audio,

    /// Video file.
    Video,

    /// Voice message.
    Voice,

    /// Video note (circular video).
    VideoNote,

    /// Document/file.
    Document,

    /// Sticker.
    Sticker,

    /// Contact card.
    Contact,

    /// Location.
    Location,
}

/// Source of attachment data.
#[derive(Debug, Clone)]
pub enum AttachmentSource {
    /// Bytes in memory.
    Bytes(Bytes),

    /// Local file path.
    Path(PathBuf),

    /// URL to download from.
    Url(String),

    /// File ID from channel (for re-sending).
    FileId(String),
}

impl Attachment {
    /// Create an attachment from bytes.
    pub fn from_bytes(
        bytes: impl Into<Bytes>,
        filename: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        let bytes = bytes.into();
        let size = bytes.len();
        let mime_type_str = mime_type.into();

        Self {
            attachment_type: Self::detect_type(&mime_type_str),
            filename: filename.into(),
            mime_type: mime_type_str,
            source: AttachmentSource::Bytes(bytes),
            size: Some(size),
            caption: None,
            spoiler: false,
        }
    }

    /// Create an attachment from a file path.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();

        if !path.exists() {
            return Err(ChannelError::Attachment(format!(
                "File not found: {:?}",
                path
            )));
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        let size = std::fs::metadata(&path).ok().map(|m| m.len() as usize);

        Ok(Self {
            attachment_type: Self::detect_type(&mime_type),
            filename,
            mime_type,
            source: AttachmentSource::Path(path),
            size,
            caption: None,
            spoiler: false,
        })
    }

    /// Create an attachment from a URL.
    pub fn from_url(url: impl Into<String>, filename: impl Into<String>) -> Self {
        let filename: String = filename.into();
        let mime_type = mime_guess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        Self {
            attachment_type: Self::detect_type(&mime_type),
            filename,
            mime_type,
            source: AttachmentSource::Url(url.into()),
            size: None,
            caption: None,
            spoiler: false,
        }
    }

    /// Create an attachment from a file ID.
    pub fn from_file_id(
        file_id: impl Into<String>,
        attachment_type: AttachmentType,
    ) -> Self {
        Self {
            attachment_type,
            filename: String::new(),
            mime_type: String::new(),
            source: AttachmentSource::FileId(file_id.into()),
            size: None,
            caption: None,
            spoiler: false,
        }
    }

    /// Set the caption.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Mark as spoiler.
    pub fn as_spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }

    /// Set the attachment type.
    pub fn with_type(mut self, attachment_type: AttachmentType) -> Self {
        self.attachment_type = attachment_type;
        self
    }

    /// Get the attachment data as bytes.
    pub async fn get_bytes(&self) -> Result<Bytes> {
        match &self.source {
            AttachmentSource::Bytes(bytes) => Ok(bytes.clone()),
            AttachmentSource::Path(path) => {
                let data = tokio::fs::read(path)
                    .await
                    .map_err(|e| ChannelError::Attachment(e.to_string()))?;
                Ok(Bytes::from(data))
            }
            AttachmentSource::Url(url) => {
                debug!("Downloading attachment from {}", url);
                let response = reqwest::get(url)
                    .await
                    .map_err(|e| ChannelError::Attachment(e.to_string()))?;
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| ChannelError::Attachment(e.to_string()))?;
                Ok(bytes)
            }
            AttachmentSource::FileId(_) => Err(ChannelError::Attachment(
                "Cannot get bytes from file ID".to_string(),
            )),
        }
    }

    /// Detect attachment type from MIME type.
    fn detect_type(mime_type: &str) -> AttachmentType {
        if mime_type.starts_with("image/") {
            AttachmentType::Image
        } else if mime_type.starts_with("audio/") {
            AttachmentType::Audio
        } else if mime_type.starts_with("video/") {
            AttachmentType::Video
        } else {
            AttachmentType::Document
        }
    }

    /// Check if this is an image.
    pub fn is_image(&self) -> bool {
        self.attachment_type == AttachmentType::Image
    }

    /// Check if this is a video.
    pub fn is_video(&self) -> bool {
        self.attachment_type == AttachmentType::Video
    }

    /// Check if this is an audio file.
    pub fn is_audio(&self) -> bool {
        matches!(
            self.attachment_type,
            AttachmentType::Audio | AttachmentType::Voice
        )
    }
}

/// Builder for attachments.
#[derive(Debug, Default)]
pub struct AttachmentBuilder {
    attachment_type: Option<AttachmentType>,
    filename: Option<String>,
    mime_type: Option<String>,
    source: Option<AttachmentSource>,
    caption: Option<String>,
    spoiler: bool,
}

impl AttachmentBuilder {
    /// Create a new attachment builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the attachment type.
    pub fn attachment_type(mut self, t: AttachmentType) -> Self {
        self.attachment_type = Some(t);
        self
    }

    /// Set the filename.
    pub fn filename(mut self, name: impl Into<String>) -> Self {
        self.filename = Some(name.into());
        self
    }

    /// Set the MIME type.
    pub fn mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    /// Set the source as bytes.
    pub fn bytes(mut self, data: impl Into<Bytes>) -> Self {
        self.source = Some(AttachmentSource::Bytes(data.into()));
        self
    }

    /// Set the source as a path.
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source = Some(AttachmentSource::Path(path.into()));
        self
    }

    /// Set the source as a URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.source = Some(AttachmentSource::Url(url.into()));
        self
    }

    /// Set the caption.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Mark as spoiler.
    pub fn spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }

    /// Build the attachment.
    pub fn build(self) -> Result<Attachment> {
        let source = self
            .source
            .ok_or_else(|| ChannelError::Attachment("Source is required".to_string()))?;

        let filename = self.filename.unwrap_or_else(|| "file".to_string());
        let mime_type = self.mime_type.unwrap_or_else(|| {
            mime_guess::from_path(&filename)
                .first_or_octet_stream()
                .to_string()
        });

        let attachment_type = self
            .attachment_type
            .unwrap_or_else(|| Attachment::detect_type(&mime_type));

        let size = match &source {
            AttachmentSource::Bytes(b) => Some(b.len()),
            _ => None,
        };

        Ok(Attachment {
            attachment_type,
            filename,
            mime_type,
            source,
            size,
            caption: self.caption,
            spoiler: self.spoiler,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_from_bytes() {
        let data = vec![0u8; 100];
        let attachment = Attachment::from_bytes(data, "test.png", "image/png");

        assert_eq!(attachment.attachment_type, AttachmentType::Image);
        assert_eq!(attachment.filename, "test.png");
        assert_eq!(attachment.size, Some(100));
    }

    #[test]
    fn test_attachment_type_detection() {
        assert_eq!(
            Attachment::detect_type("image/png"),
            AttachmentType::Image
        );
        assert_eq!(
            Attachment::detect_type("video/mp4"),
            AttachmentType::Video
        );
        assert_eq!(
            Attachment::detect_type("audio/mpeg"),
            AttachmentType::Audio
        );
        assert_eq!(
            Attachment::detect_type("application/pdf"),
            AttachmentType::Document
        );
    }

    #[test]
    fn test_attachment_builder() {
        let attachment = AttachmentBuilder::new()
            .filename("test.jpg")
            .mime_type("image/jpeg")
            .bytes(vec![0u8; 50])
            .caption("A test image")
            .spoiler()
            .build()
            .unwrap();

        assert_eq!(attachment.attachment_type, AttachmentType::Image);
        assert_eq!(attachment.caption, Some("A test image".to_string()));
        assert!(attachment.spoiler);
    }
}
