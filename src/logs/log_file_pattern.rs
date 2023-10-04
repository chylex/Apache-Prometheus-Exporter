use std::fs::DirEntry;
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use path_slash::PathExt;

/// Reads and parses an environment variable that determines the path and file name pattern of log files.
///
/// Supports 3 pattern types:
///
/// 1. A simple path to a file.
/// 2. A path with a wildcard anywhere in the file name.
/// 3. A path with a standalone wildcard component (i.e. no prefix or suffix in the folder name).
pub fn parse_log_file_pattern_from_str(pattern: &str) -> Result<LogFilePattern> {
	let pattern = Path::new(pattern).to_slash().ok_or_else(|| anyhow!("Path is invalid"))?;
	if pattern.trim().is_empty() {
		bail!("Path is empty");
	}
	
	if let Some((left, right)) = pattern.split_once('*') {
		parse_log_file_pattern_split_on_wildcard(left, right)
	} else {
		Ok(LogFilePattern::WithoutWildcard(pattern.to_string()))
	}
}

fn parse_log_file_pattern_split_on_wildcard(left: &str, right: &str) -> Result<LogFilePattern> {
	if left.contains('*') || right.contains('*') {
		bail!("Path has too many wildcards");
	}
	
	if left.ends_with('/') && right.starts_with('/') {
		return Ok(LogFilePattern::WithFolderNameWildcard(PatternWithFolderNameWildcard {
			path_prefix: left.to_string(),
			path_suffix: right[1..].to_string(),
		}));
	}
	
	if right.contains('/') {
		bail!("Path has a folder wildcard with a prefix or suffix");
	}
	
	if let Some((folder_path, file_name_prefix)) = left.rsplit_once('/') {
		Ok(LogFilePattern::WithFileNameWildcard(PatternWithFileNameWildcard {
			path: folder_path.to_string(),
			file_name_prefix: file_name_prefix.to_string(),
			file_name_suffix: right.to_string(),
		}))
	} else {
		Ok(LogFilePattern::WithFileNameWildcard(PatternWithFileNameWildcard {
			path: String::new(),
			file_name_prefix: left.to_string(),
			file_name_suffix: right.to_string(),
		}))
	}
}

#[derive(Debug)]
pub struct PatternWithFileNameWildcard {
	path: String,
	file_name_prefix: String,
	file_name_suffix: String,
}

impl PatternWithFileNameWildcard {
	fn match_wildcard<'a>(&self, file_name: &'a str) -> Option<&'a str> {
		return file_name.strip_prefix(&self.file_name_prefix).and_then(|r| r.strip_suffix(&self.file_name_suffix));
	}
	
	fn match_wildcard_on_dir_entry(&self, dir_entry: &DirEntry) -> Option<String> {
		dir_entry.file_name()
			.to_str()
			.and_then(|file_name| self.match_wildcard(file_name))
			.map(|wildcard_match| wildcard_match.to_string())
	}
}

#[derive(Debug)]
pub struct PatternWithFolderNameWildcard {
	path_prefix: String,
	path_suffix: String,
}

impl PatternWithFolderNameWildcard {
	fn match_wildcard_on_dir_entry(dir_entry: &DirEntry) -> Option<String> {
		return if matches!(dir_entry.file_type(), Ok(entry_type) if entry_type.is_dir()) {
			dir_entry.file_name().to_str().map(|s| s.into())
		} else {
			None
		};
	}
}

#[derive(Debug)]
pub enum LogFilePattern {
	WithoutWildcard(String),
	WithFileNameWildcard(PatternWithFileNameWildcard),
	WithFolderNameWildcard(PatternWithFolderNameWildcard),
}

impl LogFilePattern {
	pub fn search(&self) -> Result<Vec<LogFilePath>, io::Error> { // TODO error message
		match self {
			Self::WithoutWildcard(path) => Self::search_without_wildcard(path),
			Self::WithFileNameWildcard(pattern) => Self::search_with_file_name_wildcard(pattern),
			Self::WithFolderNameWildcard(pattern) => Self::search_with_folder_name_wildcard(pattern)
		}
	}
	
	fn search_without_wildcard(path_str: &String) -> Result<Vec<LogFilePath>, io::Error> {
		if Path::new(path_str).is_file() {
			Ok(vec![LogFilePath::with_empty_label(path_str)])
		} else {
			Err(io::Error::from(ErrorKind::NotFound))
		}
	}
	
	fn search_with_file_name_wildcard(pattern: &PatternWithFileNameWildcard) -> Result<Vec<LogFilePath>, io::Error> {
		let mut result = Vec::new();
		
		for dir_entry in Path::new(&pattern.path).read_dir()? {
			let dir_entry = dir_entry?;
			if let Some(wildcard_match) = pattern.match_wildcard_on_dir_entry(&dir_entry) {
				result.push(LogFilePath { path: dir_entry.path(), label: wildcard_match });
			}
		}
		
		Ok(result)
	}
	
	fn search_with_folder_name_wildcard(pattern: &PatternWithFolderNameWildcard) -> Result<Vec<LogFilePath>, io::Error> {
		let mut result = Vec::new();
		
		for dir_entry in Path::new(&pattern.path_prefix).read_dir()? {
			let dir_entry = dir_entry?;
			if let Some(wildcard_match) = PatternWithFolderNameWildcard::match_wildcard_on_dir_entry(&dir_entry) {
				let full_path = dir_entry.path().join(&pattern.path_suffix);
				if full_path.is_file() {
					result.push(LogFilePath { path: full_path, label: wildcard_match })
				}
			}
		}
		
		Ok(result)
	}
}

pub struct LogFilePath {
	pub path: PathBuf,
	pub label: String,
}

impl LogFilePath {
	fn with_empty_label(s: &String) -> LogFilePath {
		LogFilePath {
			path: PathBuf::from(s),
			label: String::default(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{LogFilePattern, parse_log_file_pattern_from_str};
	
	#[test]
	fn empty_path() {
		assert!(matches!(parse_log_file_pattern_from_str(""), Err(err) if err.to_string() == "Path is empty"));
		assert!(matches!(parse_log_file_pattern_from_str("  "), Err(err) if err.to_string() == "Path is empty"));
	}
	
	#[test]
	fn too_many_wildcards() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/*/to/files/*.log"), Err(err) if err.to_string() == "Path has too many wildcards"));
	}
	
	#[test]
	fn folder_wildcard_with_prefix_not_supported() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/*abc/to/files/access.log"), Err(err) if err.to_string() == "Path has a folder wildcard with a prefix or suffix"));
	}
	
	#[test]
	fn folder_wildcard_with_suffix_not_supported() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/abc*/to/files/access.log"), Err(err) if err.to_string() == "Path has a folder wildcard with a prefix or suffix"));
	}
	
	#[test]
	fn valid_without_wildcard() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/to/file/access.log"), Ok(LogFilePattern::WithoutWildcard(path)) if path == "/path/to/file/access.log"));
	}
	
	#[test]
	fn valid_with_file_name_wildcard_prefix() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/to/files/access_*"), Ok(LogFilePattern::WithFileNameWildcard(pattern)) if pattern.path == "/path/to/files" && pattern.file_name_prefix == "access_" && pattern.file_name_suffix.is_empty()));
	}
	
	#[test]
	fn valid_with_file_name_wildcard_suffix() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/to/files/*_access.log"), Ok(LogFilePattern::WithFileNameWildcard(pattern)) if pattern.path == "/path/to/files" && pattern.file_name_prefix.is_empty() && pattern.file_name_suffix == "_access.log"));
	}
	
	#[test]
	fn valid_with_file_name_wildcard_both() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/to/files/access_*.log"), Ok(LogFilePattern::WithFileNameWildcard(pattern)) if pattern.path == "/path/to/files" && pattern.file_name_prefix == "access_" && pattern.file_name_suffix == ".log"));
	}
	
	#[test]
	fn valid_with_folder_wildcard() {
		assert!(matches!(parse_log_file_pattern_from_str("/path/to/*/files/access.log"), Ok(LogFilePattern::WithFolderNameWildcard(pattern)) if pattern.path_prefix == "/path/to/" && pattern.path_suffix == "files/access.log"));
	}
}
