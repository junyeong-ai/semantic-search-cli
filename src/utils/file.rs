//! File utilities for indexing operations.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

/// Calculate SHA-256 checksum of content.
pub fn calculate_checksum(content: &str) -> String {
    let hash = Sha256::digest(content.as_bytes());
    hex::encode(hash)
}

/// Calculate SHA-256 checksum of a file.
pub fn calculate_file_checksum(path: &Path) -> std::io::Result<String> {
    let content = fs::read_to_string(path)?;
    Ok(calculate_checksum(&content))
}

/// Check if a file is likely a text file.
pub fn is_text_file(path: &Path) -> bool {
    // Check by extension
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if is_binary_extension(&ext) {
            return false;
        }
        if is_text_extension(&ext) {
            return true;
        }
    }

    // Check by reading first bytes
    if let Ok(file) = fs::File::open(path) {
        let mut buffer = [0u8; 512];
        let mut reader = std::io::BufReader::new(file);
        if let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                return true; // Empty file is text
            }
            // Check for null bytes (binary indicator)
            if buffer[..n].contains(&0) {
                return false;
            }
            return true;
        }
    }

    false
}

/// Read file content with size limit.
pub fn read_file_content(path: &Path, max_size: u64) -> std::io::Result<String> {
    let metadata = fs::metadata(path)?;

    if metadata.len() > max_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file exceeds maximum size: {} > {}",
                metadata.len(),
                max_size
            ),
        ));
    }

    fs::read_to_string(path)
}

/// Check if extension indicates a binary file.
fn is_binary_extension(ext: &str) -> bool {
    matches!(
        ext,
        "exe"
            | "dll"
            | "so"
            | "dylib"
            | "a"
            | "o"
            | "obj"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "ico"
            | "webp"
            | "svg"
            | "mp3"
            | "mp4"
            | "avi"
            | "mkv"
            | "mov"
            | "wav"
            | "flac"
            | "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "xz"
            | "7z"
            | "rar"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "class"
            | "jar"
            | "pyc"
            | "pyo"
            | "db"
            | "sqlite"
            | "sqlite3"
            | "bin"
            | "dat"
            | "pak"
    )
}

/// Check if extension indicates a text file.
fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        // Source code
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "go" | "java" | "kt" | "kts"
            | "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "hh"
            | "rb" | "php" | "swift" | "scala" | "clj" | "cljs" | "erl" | "ex" | "exs"
            | "hs" | "ml" | "fs" | "fsi" | "fsx"
            | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd"
            | "lua" | "pl" | "pm" | "r" | "R" | "jl"
            // Web
            | "html" | "htm" | "css" | "scss" | "sass" | "less"
            | "vue" | "svelte" | "astro"
            // Data/Config
            | "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "cfg"
            | "env" | "properties" | "conf"
            // Documentation
            | "md" | "markdown" | "rst" | "txt" | "adoc" | "org"
            // Other
            | "sql" | "graphql" | "gql" | "prisma"
            | "dockerfile" | "makefile" | "justfile"
            | "gitignore" | "gitattributes" | "editorconfig"
    )
}

/// Get the relative path from a base directory.
pub fn get_relative_path(base: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(base)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Sanitize a filename by replacing invalid characters.
///
/// Replaces characters that are not allowed in filenames on common operating
/// systems (Windows, macOS, Linux) with hyphens.
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_calculate_checksum() {
        let checksum = calculate_checksum("hello world");
        assert_eq!(checksum.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn test_is_binary_extension() {
        assert!(is_binary_extension("exe"));
        assert!(is_binary_extension("png"));
        assert!(!is_binary_extension("rs"));
        assert!(!is_binary_extension("md"));
    }

    #[test]
    fn test_is_text_extension() {
        assert!(is_text_extension("rs"));
        assert!(is_text_extension("py"));
        assert!(is_text_extension("md"));
        assert!(!is_text_extension("png"));
    }

    #[test]
    fn test_is_text_file() {
        let path = PathBuf::from("test.rs");
        assert!(is_text_file(&path));

        let path = PathBuf::from("test.png");
        assert!(!is_text_file(&path));
    }
}
