//! Document parsers for extracting text from various file formats.
//!
//! Parsers convert raw document files into `ParsedDocument` structs containing
//! plain text content and metadata. Feature-gated parsers are available for
//! PDF, DOCX, and EPUB formats.

use crate::error::LearningsError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

/// Supported document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Epub,
    Markdown,
    Txt,
}

impl std::fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentFormat::Pdf => write!(f, "pdf"),
            DocumentFormat::Docx => write!(f, "docx"),
            DocumentFormat::Epub => write!(f, "epub"),
            DocumentFormat::Markdown => write!(f, "markdown"),
            DocumentFormat::Txt => write!(f, "txt"),
        }
    }
}

/// A parsed document with extracted text and metadata.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// Document title (from metadata or filename).
    pub title: String,
    /// Extracted plain text content.
    pub content: String,
    /// Additional metadata (author, date, etc.).
    pub metadata: HashMap<String, String>,
}

/// Trait for document parsers.
#[async_trait]
pub trait DocumentParser: Send + Sync {
    /// Get the formats this parser supports.
    fn supported_formats(&self) -> &[DocumentFormat];

    /// Parse a document at the given path.
    async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError>;
}

/// Markdown document parser.
pub struct MarkdownParser;

impl MarkdownParser {
    /// Create a new markdown parser.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentParser for MarkdownParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Markdown]
    }

    async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| LearningsError::Io(e))?;

        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let mut metadata = HashMap::new();

        // Extract frontmatter if present (YAML between --- delimiters)
        let content = if content.starts_with("---\n") {
            if let Some(end) = content[3..].find("\n---\n") {
                let frontmatter = &content[3..end + 3];
                for line in frontmatter.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        metadata.insert(key.trim().to_string(), value.trim().to_string());
                    }
                }
                content[end + 7..].to_string() // Skip frontmatter
            } else {
                content
            }
        } else {
            content
        };

        // Extract title from first H1 heading if present
        let title = if let Some(line) = content.lines().next() {
            let trimmed = line.trim();
            if trimmed.starts_with("# ") {
                trimmed[2..].trim().to_string()
            } else {
                title
            }
        } else {
            title
        };

        Ok(ParsedDocument {
            title,
            content,
            metadata,
        })
    }
}

/// Plain text document parser.
pub struct TextParser;

impl TextParser {
    /// Create a new text parser.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentParser for TextParser {
    fn supported_formats(&self) -> &[DocumentFormat] {
        &[DocumentFormat::Txt]
    }

    async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| LearningsError::Io(e))?;

        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        Ok(ParsedDocument {
            title,
            content,
            metadata: HashMap::new(),
        })
    }
}

/// Parser kind enum for dispatching without dyn trait.
enum ParserKind {
    Markdown(MarkdownParser),
    Text(TextParser),
    #[cfg(feature = "pdf-parser")]
    Pdf(crate::parser_pdf::PdfParser),
    #[cfg(feature = "docx-parser")]
    Docx(crate::parser_docx::DocxParser),
    #[cfg(feature = "epub-parser")]
    Epub(crate::parser_epub::EpubParser),
}

/// Composite parser that delegates to the appropriate parser based on file extension.
pub struct CompositeParser {
    parsers: Vec<ParserKind>,
}

impl CompositeParser {
    /// Create a composite parser with all available parsers.
    pub fn new() -> Self {
        let parsers: Vec<ParserKind> = vec![
            ParserKind::Markdown(MarkdownParser::new()),
            ParserKind::Text(TextParser::new()),
        ];

        #[cfg(feature = "pdf-parser")]
        parsers.push(ParserKind::Pdf(crate::parser_pdf::PdfParser::new()));

        #[cfg(feature = "docx-parser")]
        parsers.push(ParserKind::Docx(crate::parser_docx::DocxParser::new()));

        #[cfg(feature = "epub-parser")]
        parsers.push(ParserKind::Epub(crate::parser_epub::EpubParser::new()));

        Self { parsers }
    }

    /// Parse a document by detecting its format from the extension.
    pub async fn parse_file(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
        let format = detect_format(path);
        for parser in &self.parsers {
            let supported = match parser {
                ParserKind::Markdown(p) => p.supported_formats(),
                ParserKind::Text(p) => p.supported_formats(),
                #[cfg(feature = "pdf-parser")]
                ParserKind::Pdf(p) => p.supported_formats(),
                #[cfg(feature = "docx-parser")]
                ParserKind::Docx(p) => p.supported_formats(),
                #[cfg(feature = "epub-parser")]
                ParserKind::Epub(p) => p.supported_formats(),
            };
            if supported.contains(&format) {
                return match parser {
                    ParserKind::Markdown(p) => p.parse(path).await,
                    ParserKind::Text(p) => p.parse(path).await,
                    #[cfg(feature = "pdf-parser")]
                    ParserKind::Pdf(p) => p.parse(path).await,
                    #[cfg(feature = "docx-parser")]
                    ParserKind::Docx(p) => p.parse(path).await,
                    #[cfg(feature = "epub-parser")]
                    ParserKind::Epub(p) => p.parse(path).await,
                };
            }
        }

        // Fallback: try as plain text
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| LearningsError::Io(e))?;
        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        Ok(ParsedDocument {
            title,
            content,
            metadata: HashMap::new(),
        })
    }
}

impl Default for CompositeParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect document format from file extension.
pub fn detect_format(path: &Path) -> DocumentFormat {
    match path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
        Some(ext) => match ext.as_str() {
            "pdf" => DocumentFormat::Pdf,
            "docx" | "doc" => DocumentFormat::Docx,
            "epub" => DocumentFormat::Epub,
            "md" | "markdown" => DocumentFormat::Markdown,
            _ => DocumentFormat::Txt,
        },
        None => DocumentFormat::Txt,
    }
}

// Feature-gated parser modules

#[cfg(feature = "pdf-parser")]
pub mod parser_pdf {
    use super::*;

    /// PDF document parser (feature-gated).
    pub struct PdfParser;

    impl PdfParser {
        /// Create a new PDF parser.
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for PdfParser {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl DocumentParser for PdfParser {
        fn supported_formats(&self) -> &[DocumentFormat] {
            &[DocumentFormat::Pdf]
        }

        async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
            let path_str = path.to_string_lossy().to_string();
            let content = tokio::task::spawn_blocking(move || {
                pdf_extract::extract_text(&path_str)
                    .map_err(|e| LearningsError::DocumentParse(format!("PDF parsing error: {}", e)))
            })
            .await
            .map_err(|e| LearningsError::DocumentParse(format!("PDF task error: {}", e)))??;

            let title = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();

            Ok(ParsedDocument {
                title,
                content,
                metadata: HashMap::new(),
            })
        }
    }
}

#[cfg(feature = "docx-parser")]
pub mod parser_docx {
    use super::*;

    /// DOCX document parser (feature-gated).
    pub struct DocxParser;

    impl DocxParser {
        /// Create a new DOCX parser.
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for DocxParser {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl DocumentParser for DocxParser {
        fn supported_formats(&self) -> &[DocumentFormat] {
            &[DocumentFormat::Docx]
        }

        async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
            let path_buf = path.to_path_buf();
            let content = tokio::task::spawn_blocking(move || {
                // Read the docx file and extract text from paragraphs
                let buf = std::fs::read(&path_buf)
                    .map_err(|e| LearningsError::Io(e))?;
                let docx = docx_rs::read_docx(&buf)
                    .map_err(|e| LearningsError::DocumentParse(format!("DOCX parsing error: {}", e)))?;
                let mut text = String::new();
                for child in &docx.children {
                    if let docx_rs::DocumentChild::Paragraph(para) = child {
                        for run in &para.children {
                            if let docx_rs::ParagraphChild::Run(r) = run {
                                for run_child in &r.children {
                                    if let docx_rs::RunChild::Text(t) = run_child {
                                        text.push_str(&t.value);
                                        text.push(' ');
                                    }
                                }
                            }
                        }
                        text.push('\n');
                    }
                }
                Ok::<String, LearningsError>(text)
            })
            .await
            .map_err(|e| LearningsError::DocumentParse(format!("DOCX task error: {}", e)))??;

            let title = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();

            Ok(ParsedDocument {
                title,
                content,
                metadata: HashMap::new(),
            })
        }
    }
}

#[cfg(feature = "epub-parser")]
pub mod parser_epub {
    use super::*;

    /// EPUB document parser (feature-gated).
    pub struct EpubParser;

    impl EpubParser {
        /// Create a new EPUB parser.
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for EpubParser {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl DocumentParser for EpubParser {
        fn supported_formats(&self) -> &[DocumentFormat] {
            &[DocumentFormat::Epub]
        }

        async fn parse(&self, path: &Path) -> Result<ParsedDocument, LearningsError> {
            let path_buf = path.to_path_buf();
            let content = tokio::task::spawn_blocking(move || {
                let mut epub = epub::EpubDoc::new(&path_buf)
                    .map_err(|e| LearningsError::DocumentParse(format!("EPUB open error: {}", e)))?;

                let title = epub.metadata.get("title")
                    .and_then(|v| v.first())
                    .cloned()
                    .unwrap_or_else(|| "Untitled".to_string());

                let mut text = String::new();
                let num_pages = epub.get_num_pages();
                for i in 0..num_pages {
                    if let Ok(chapter) = epub.get_chapter(i) {
                        // Strip HTML tags for plain text
                        let plain = strip_html_tags(&String::from_utf8_lossy(&chapter));
                        text.push_str(&plain);
                        text.push('\n');
                    }
                }

                Ok::<(String, String), LearningsError>((title, text))
            })
            .await
            .map_err(|e| LearningsError::DocumentParse(format!("EPUB task error: {}", e)))??;

            Ok(ParsedDocument {
                title: content.0,
                content: content.1,
                metadata: HashMap::new(),
            })
        }
    }

    /// Basic HTML tag stripping.
    fn strip_html_tags(html: &str) -> String {
        let re = regex::Regex::new(r"<[^>]+>").unwrap();
        re.replace_all(html, "").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format(Path::new("doc.pdf")), DocumentFormat::Pdf);
        assert_eq!(detect_format(Path::new("doc.docx")), DocumentFormat::Docx);
        assert_eq!(detect_format(Path::new("book.epub")), DocumentFormat::Epub);
        assert_eq!(detect_format(Path::new("readme.md")), DocumentFormat::Markdown);
        assert_eq!(detect_format(Path::new("notes.txt")), DocumentFormat::Txt);
        assert_eq!(detect_format(Path::new("unknown.xyz")), DocumentFormat::Txt);
    }

    #[tokio::test]
    async fn test_markdown_parser() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "# My Title\n\nSome content here.\n\n## Section\n\nMore content.").await.unwrap();

        let parser = MarkdownParser::new();
        let doc = parser.parse(&file_path).await.unwrap();
        assert_eq!(doc.title, "My Title");
        assert!(doc.content.contains("Some content here."));
        assert!(doc.content.contains("Section"));
    }

    #[tokio::test]
    async fn test_markdown_parser_with_frontmatter() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("frontmatter.md");
        fs::write(&file_path, "---\ntitle: Custom Title\nauthor: Test\n---\n\n# Heading\n\nContent.").await.unwrap();

        let parser = MarkdownParser::new();
        let doc = parser.parse(&file_path).await.unwrap();
        assert_eq!(doc.metadata.get("title"), Some(&"Custom Title".to_string()));
        assert_eq!(doc.metadata.get("author"), Some(&"Test".to_string()));
    }

    #[tokio::test]
    async fn test_text_parser() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("notes.txt");
        fs::write(&file_path, "Plain text content here.").await.unwrap();

        let parser = TextParser::new();
        let doc = parser.parse(&file_path).await.unwrap();
        assert_eq!(doc.title, "notes");
        assert_eq!(doc.content, "Plain text content here.");
    }

    #[tokio::test]
    async fn test_composite_parser() {
        let dir = TempDir::new().unwrap();
        let md_path = dir.path().join("doc.md");
        let txt_path = dir.path().join("doc.txt");

        fs::write(&md_path, "# Title\n\nMarkdown content.").await.unwrap();
        fs::write(&txt_path, "Text content.").await.unwrap();

        let parser = CompositeParser::new();

        let md_doc = parser.parse_file(&md_path).await.unwrap();
        assert_eq!(md_doc.title, "Title");

        let txt_doc = parser.parse_file(&txt_path).await.unwrap();
        assert_eq!(txt_doc.title, "doc");
    }

    #[test]
    fn test_document_format_display() {
        assert_eq!(DocumentFormat::Pdf.to_string(), "pdf");
        assert_eq!(DocumentFormat::Markdown.to_string(), "markdown");
        assert_eq!(DocumentFormat::Txt.to_string(), "txt");
    }
}