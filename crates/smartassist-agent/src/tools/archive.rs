//! Archive manipulation tools (zip, tar, gzip).

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;
use zip::write::SimpleFileOptions;

/// Tool for creating and extracting zip archives.
pub struct ZipTool;

impl ZipTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZipTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ZipArgs {
    /// Operation: "create" or "extract"
    operation: String,
    /// Path to the archive file
    archive: String,
    /// Files to add (for create) or output directory (for extract)
    path: Option<String>,
    /// Files to include in the archive (for create)
    files: Option<Vec<String>>,
}

#[async_trait]
impl Tool for ZipTool {
    fn name(&self) -> &str {
        "zip"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "zip".to_string(),
            description: "Create or extract zip archives".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["create", "extract", "list"],
                        "description": "Operation to perform"
                    },
                    "archive": {
                        "type": "string",
                        "description": "Path to the zip archive"
                    },
                    "path": {
                        "type": "string",
                        "description": "Output directory (extract) or base path (create)"
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Files to add (for create operation)"
                    }
                },
                "required": ["operation", "archive"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: ZipArgs = serde_json::from_value(args)?;

        let archive_path = if Path::new(&args.archive).is_absolute() {
            args.archive.clone()
        } else {
            context.cwd.join(&args.archive).to_string_lossy().to_string()
        };

        match args.operation.as_str() {
            "create" => {
                let files = args.files.unwrap_or_default();
                if files.is_empty() {
                    return Ok(ToolResult::error(
                        tool_use_id,
                        "No files specified for archive creation".to_string(),
                    ));
                }

                let file = File::create(&archive_path)?;
                let mut zip = zip::ZipWriter::new(file);
                let options = SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                let base_path = args.path
                    .as_ref()
                    .map(|p| context.cwd.join(p))
                    .unwrap_or_else(|| context.cwd.clone());

                let mut added = 0;
                for file_path in &files {
                    let full_path = if Path::new(file_path).is_absolute() {
                        std::path::PathBuf::from(file_path)
                    } else {
                        base_path.join(file_path)
                    };

                    if full_path.is_file() {
                        let name = full_path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy();

                        zip.start_file(name.to_string(), options)
                            .map_err(|e| crate::error::AgentError::tool_execution(format!("Zip error: {}", e)))?;
                        let mut f = File::open(&full_path)?;
                        let mut buffer = Vec::new();
                        f.read_to_end(&mut buffer)?;
                        zip.write_all(&buffer)?;
                        added += 1;
                    }
                }

                zip.finish()
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Zip error: {}", e)))?;

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "archive": archive_path,
                        "files_added": added
                    }),
                ).with_duration(start.elapsed()))
            }
            "extract" => {
                let output_dir = args.path
                    .as_ref()
                    .map(|p| {
                        if Path::new(p).is_absolute() {
                            std::path::PathBuf::from(p)
                        } else {
                            context.cwd.join(p)
                        }
                    })
                    .unwrap_or_else(|| context.cwd.clone());

                std::fs::create_dir_all(&output_dir)?;

                let file = File::open(&archive_path)?;
                let mut archive = zip::ZipArchive::new(file)
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Zip error: {}", e)))?;
                let mut extracted = Vec::new();

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i)
                        .map_err(|e| crate::error::AgentError::tool_execution(format!("Zip error: {}", e)))?;
                    let outpath = output_dir.join(file.mangled_name());

                    if file.name().ends_with('/') {
                        std::fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            std::fs::create_dir_all(p)?;
                        }
                        let mut outfile = File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                        extracted.push(outpath.to_string_lossy().to_string());
                    }
                }

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "output_dir": output_dir.to_string_lossy(),
                        "files_extracted": extracted.len(),
                        "files": extracted
                    }),
                ).with_duration(start.elapsed()))
            }
            "list" => {
                let file = File::open(&archive_path)?;
                let mut archive = zip::ZipArchive::new(file)
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Zip error: {}", e)))?;

                let entries: Vec<ArchiveEntry> = (0..archive.len())
                    .filter_map(|i| {
                        archive.by_index(i).ok().map(|f| ArchiveEntry {
                            name: f.name().to_string(),
                            size: f.size(),
                            compressed_size: f.compressed_size(),
                            is_dir: f.is_dir(),
                        })
                    })
                    .collect();

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({ "entries": entries }),
                ).with_duration(start.elapsed()))
            }
            _ => Ok(ToolResult::error(
                tool_use_id,
                format!("Unknown operation: {}. Use 'create', 'extract', or 'list'", args.operation),
            )),
        }
    }
}

#[derive(Debug, Serialize)]
struct ArchiveEntry {
    name: String,
    size: u64,
    compressed_size: u64,
    is_dir: bool,
}

/// Tool for creating and extracting tar archives.
pub struct TarTool;

impl TarTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TarTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TarArgs {
    /// Operation: "create" or "extract"
    operation: String,
    /// Path to the archive file
    archive: String,
    /// Files to add (for create) or output directory (for extract)
    path: Option<String>,
    /// Files to include in the archive (for create)
    files: Option<Vec<String>>,
    /// Compression type: "none", "gzip", or "xz"
    compression: Option<String>,
}

#[async_trait]
impl Tool for TarTool {
    fn name(&self) -> &str {
        "tar"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "tar".to_string(),
            description: "Create or extract tar archives (with optional gzip compression)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["create", "extract", "list"],
                        "description": "Operation to perform"
                    },
                    "archive": {
                        "type": "string",
                        "description": "Path to the tar archive"
                    },
                    "path": {
                        "type": "string",
                        "description": "Output directory (extract) or base path (create)"
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Files to add (for create operation)"
                    },
                    "compression": {
                        "type": "string",
                        "enum": ["none", "gzip"],
                        "description": "Compression type (default: auto-detect from extension)"
                    }
                },
                "required": ["operation", "archive"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: TarArgs = serde_json::from_value(args)?;

        let archive_path = if Path::new(&args.archive).is_absolute() {
            std::path::PathBuf::from(&args.archive)
        } else {
            context.cwd.join(&args.archive)
        };

        // Auto-detect compression from extension
        let use_gzip = args.compression
            .as_ref()
            .map(|c| c == "gzip")
            .unwrap_or_else(|| {
                archive_path.extension()
                    .map(|e| e == "gz" || e == "tgz")
                    .unwrap_or(false)
            });

        match args.operation.as_str() {
            "create" => {
                let files = args.files.unwrap_or_default();
                if files.is_empty() {
                    return Ok(ToolResult::error(
                        tool_use_id,
                        "No files specified for archive creation".to_string(),
                    ));
                }

                let file = File::create(&archive_path)?;
                let mut added = 0;

                let base_path = args.path
                    .as_ref()
                    .map(|p| context.cwd.join(p))
                    .unwrap_or_else(|| context.cwd.clone());

                if use_gzip {
                    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
                    let mut ar = tar::Builder::new(encoder);

                    for file_path in &files {
                        let full_path = if Path::new(file_path).is_absolute() {
                            std::path::PathBuf::from(file_path)
                        } else {
                            base_path.join(file_path)
                        };

                        if full_path.is_file() {
                            let name = full_path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            ar.append_path_with_name(&full_path, &*name)?;
                            added += 1;
                        } else if full_path.is_dir() {
                            let name = full_path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            ar.append_dir_all(&*name, &full_path)?;
                            added += 1;
                        }
                    }

                    ar.finish()?;
                } else {
                    let mut ar = tar::Builder::new(file);

                    for file_path in &files {
                        let full_path = if Path::new(file_path).is_absolute() {
                            std::path::PathBuf::from(file_path)
                        } else {
                            base_path.join(file_path)
                        };

                        if full_path.is_file() {
                            let name = full_path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            ar.append_path_with_name(&full_path, &*name)?;
                            added += 1;
                        } else if full_path.is_dir() {
                            let name = full_path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            ar.append_dir_all(&*name, &full_path)?;
                            added += 1;
                        }
                    }

                    ar.finish()?;
                }

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "archive": archive_path.to_string_lossy(),
                        "entries_added": added,
                        "compression": if use_gzip { "gzip" } else { "none" }
                    }),
                ).with_duration(start.elapsed()))
            }
            "extract" => {
                let output_dir = args.path
                    .as_ref()
                    .map(|p| {
                        if Path::new(p).is_absolute() {
                            std::path::PathBuf::from(p)
                        } else {
                            context.cwd.join(p)
                        }
                    })
                    .unwrap_or_else(|| context.cwd.clone());

                std::fs::create_dir_all(&output_dir)?;

                let file = File::open(&archive_path)?;
                let mut extracted = Vec::new();

                if use_gzip {
                    let decoder = flate2::read::GzDecoder::new(file);
                    let mut ar = tar::Archive::new(decoder);

                    for entry in ar.entries()? {
                        let mut entry = entry?;
                        let path = entry.path()?.to_path_buf();
                        let outpath = output_dir.join(&path);
                        entry.unpack(&outpath)?;
                        extracted.push(outpath.to_string_lossy().to_string());
                    }
                } else {
                    let mut ar = tar::Archive::new(file);

                    for entry in ar.entries()? {
                        let mut entry = entry?;
                        let path = entry.path()?.to_path_buf();
                        let outpath = output_dir.join(&path);
                        entry.unpack(&outpath)?;
                        extracted.push(outpath.to_string_lossy().to_string());
                    }
                }

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({
                        "output_dir": output_dir.to_string_lossy(),
                        "files_extracted": extracted.len(),
                        "files": extracted
                    }),
                ).with_duration(start.elapsed()))
            }
            "list" => {
                let file = File::open(&archive_path)?;
                let mut entries = Vec::new();

                if use_gzip {
                    let decoder = flate2::read::GzDecoder::new(file);
                    let mut ar = tar::Archive::new(decoder);

                    for entry in ar.entries()? {
                        let entry = entry?;
                        entries.push(TarEntry {
                            name: entry.path()?.to_string_lossy().to_string(),
                            size: entry.size(),
                            is_dir: entry.header().entry_type().is_dir(),
                        });
                    }
                } else {
                    let mut ar = tar::Archive::new(file);

                    for entry in ar.entries()? {
                        let entry = entry?;
                        entries.push(TarEntry {
                            name: entry.path()?.to_string_lossy().to_string(),
                            size: entry.size(),
                            is_dir: entry.header().entry_type().is_dir(),
                        });
                    }
                }

                Ok(ToolResult::success(
                    tool_use_id,
                    json!({ "entries": entries }),
                ).with_duration(start.elapsed()))
            }
            _ => Ok(ToolResult::error(
                tool_use_id,
                format!("Unknown operation: {}. Use 'create', 'extract', or 'list'", args.operation),
            )),
        }
    }
}

#[derive(Debug, Serialize)]
struct TarEntry {
    name: String,
    size: u64,
    is_dir: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_zip_create_and_extract() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = ZipTool::new();

        // Create archive
        let result = tool.execute(
            "test",
            json!({
                "operation": "create",
                "archive": "test.zip",
                "files": ["test.txt"]
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(temp.path().join("test.zip").exists());

        // Extract archive
        let extract_dir = temp.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();

        let result = tool.execute(
            "test",
            json!({
                "operation": "extract",
                "archive": "test.zip",
                "path": "extracted"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(extract_dir.join("test.txt").exists());
    }

    #[tokio::test]
    async fn test_zip_list() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = ZipTool::new();

        // Create archive first
        tool.execute(
            "test",
            json!({
                "operation": "create",
                "archive": "test.zip",
                "files": ["test.txt"]
            }),
            &context,
        ).await.unwrap();

        // List contents
        let result = tool.execute(
            "test",
            json!({
                "operation": "list",
                "archive": "test.zip"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_tar_create_and_extract() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = TarTool::new();

        // Create archive
        let result = tool.execute(
            "test",
            json!({
                "operation": "create",
                "archive": "test.tar",
                "files": ["test.txt"]
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(temp.path().join("test.tar").exists());

        // Extract archive
        let extract_dir = temp.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();

        let result = tool.execute(
            "test",
            json!({
                "operation": "extract",
                "archive": "test.tar",
                "path": "extracted"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(extract_dir.join("test.txt").exists());
    }

    #[tokio::test]
    async fn test_tar_gz_create_and_extract() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = TarTool::new();

        // Create gzipped archive
        let result = tool.execute(
            "test",
            json!({
                "operation": "create",
                "archive": "test.tar.gz",
                "files": ["test.txt"]
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(temp.path().join("test.tar.gz").exists());

        // Extract gzipped archive
        let extract_dir = temp.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();

        let result = tool.execute(
            "test",
            json!({
                "operation": "extract",
                "archive": "test.tar.gz",
                "path": "extracted"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(extract_dir.join("test.txt").exists());
    }
}
