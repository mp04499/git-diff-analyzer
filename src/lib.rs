//! This library provides functionality for parsing Git repositories,
//! analyzing code changes, and generating insights for code review.

#![warn(missing_docs)]

use std::collections::{HashSet};
use std::ffi::{CStr, CString};
use similar::{Algorithm, TextDiff};
use std::fs::remove_dir_all;
use git2::{Repository, build};
use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};
use tracing::{info, warn, debug};
use dashmap::DashMap;
use tree_sitter;
use libc::c_char;

/// Represents a code change in a file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeChange {
    /// Path to the file that changed
    pub file_path: String,
    /// Original content of the file
    pub old_content: String,
    /// New content of the file
    pub new_content: String,
    /// Individual line changes
    pub line_changes: Vec<LineChange>,
    /// Complexity metrics
    pub complexity: ComplexityMetrics,
}

/// Represents a change to a specific line
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LineChange {
    /// Line number in the file
    pub line_a: Option<u32>,
    /// Line number in the file
    pub line_b: Option<u32>,
    /// Type of change (added, removed, modified)
    pub change_type: ChangeType,
    /// Content of the line
    pub content: String,
    // pub context: LineContext,
}

/// Type of change made to a line
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ChangeType {
    /// Line was added
    Added,
    /// Line was removed
    Removed,
    /// Line was modified
    Modified,
    /// Line was unchanged
    Equal,
}

/// Contextual information about a line change
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LineContext {
    /// Function or method name containing this line
    pub function_name: Option<String>,
    /// Class or struct name containing this line
    pub class_name: Option<String>,
    /// Import statements or dependencies affected
    pub imports_affected: Vec<String>,
}

/// Code complexity metrics
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ComplexityMetrics {
    /// Cyclomatic complexity
    pub cyclomatic_complexity: u32,
    /// Number of lines of code
    pub lines_of_code: u32,
    /// Number of functions/methods
    pub function_count: u32,
    /// Nesting depth
    pub max_nesting_depth: u32,
}

/// Configuration for the analysis engine
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Maximum number of files to process concurrently
    pub max_concurrent_files: usize,
    /// Whether to include binary files in analysis
    pub include_binary_files: bool,
    /// File extensions to analyze
    pub supported_extensions: Vec<String>,
    /// Maximum file size to analyze (in bytes)
    pub max_file_size: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            max_concurrent_files: num_cpus::get(),
            include_binary_files: false,
            supported_extensions: vec![
                "rs".to_string(),
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "go".to_string(),
                "java".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "h".to_string(),
                "tsx".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Main analysis engine
pub struct CodeAnalysisEngine {
    config: AnalysisConfig,
    parsers: DashMap<String, tree_sitter::Parser>,
}

impl CodeAnalysisEngine {
    /// Create a new analysis engine with default configuration
    pub fn new() -> Self {
        Self::with_config(AnalysisConfig::default())
    }

    /// Create a new analysis engine with custom configuration
    pub fn with_config(config: AnalysisConfig) -> Self {
        let parsers = DashMap::new();

        // Initialize parsers for supported languages
        Self::init_parsers(&parsers);

        Self { config, parsers }
    }

    fn init_parsers(parsers: &DashMap<String, tree_sitter::Parser>) {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        // Rust parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("rs".to_string(), parser);
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        // Python parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("py".to_string(), parser);
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_javascript::LANGUAGE;
        // JavaScript parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("js".to_string(), parser);
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
        // TypeScript parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("ts".to_string(), parser);
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TSX;
        // TypeScript parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("tsx".to_string(), parser);
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_go::LANGUAGE;
        // Go parser
        if parser.set_language(&language.into()).is_ok() {
            parsers.insert("go".to_string(), parser);
        }

        // let mut parser = tree_sitter::Parser::new();
        // let language = tree_sitter_json::LANGUAGE;
        // // JSON parser
        // if parser.set_language(&language.into()).is_ok() {
        //     parsers.insert("json".to_string(), parser);
        // }
    }

    /// Analyze Git diff between two commits
    pub fn analyze_git_diff(&self, repo_path: &Path, commit_hash: &str, url: &str) -> Result<Vec<CodeChange>> {
        info!("Starting analysis of repository: {}", repo_path.display());

        let repo = match build::RepoBuilder::new().branch("dev").clone(url, repo_path) {
            Ok(repo) => repo,
            Err(e) => {
                warn!("Failed to open repository at {}: {}", repo_path.display(), e);
                return Err(anyhow::anyhow!("Failed to open repository at {}", repo_path.display()));
            }
        };

        let commit = repo.find_commit(git2::Oid::from_str(commit_hash)?)
            .with_context(|| format!("Failed to find commit {}", commit_hash))?;

        let parent = commit.parent(0)
            .with_context(|| "Failed to get parent commit")?;

        let parent_tree = parent.tree()?;
        let commit_tree = commit.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)?;

        let mut changes = Vec::new();
        let changes_map = DashMap::new();

        // Process diff in parallel
        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    if let Some(path_str) = path.to_str() {
                        println!("Path Str: {}", path_str);
                        if self.should_analyze_file(path_str) {
                            println!("Analysing file");
                            debug!("Processing file: {}", path_str);

                            // Get file content
                            let old_content = self.get_file_content(&repo, &parent_tree, path_str);
                            let new_content = self.get_file_content(&repo, &commit_tree, path_str);

                            if let (Ok(old), Ok(new)) = (old_content, new_content) {
                                let change = self.analyze_file_change(path_str, &old, &new);
                                changes_map.insert(path_str.to_string(), change);
                            }
                        }
                    }
                }
                true
            },
            None,
            None,
            None,
        )?;

        // Convert to Vec
        changes.extend(changes_map.into_iter().map(|(_, change)| change));

        info!("Analysis complete. Found {} changed files", changes.len());
        remove_dir_all(repo_path)?;
        Ok(changes)
    }

    fn should_analyze_file(&self, file_path: &str) -> bool {
        if let Some(extension) = Path::new(file_path).extension() {
            println!("File extension: {}", extension.to_string_lossy());
            if let Some(ext_str) = extension.to_str() {
                println!("Checking if extension is supported: {}", ext_str);
                return self.config.supported_extensions.contains(&ext_str.to_string());
            }
        }
        false
    }

    fn get_file_content(&self, repo: &Repository, tree: &git2::Tree, path: &str) -> Result<String> {
        let entry = tree.get_path(Path::new(path))?;
        let object = entry.to_object(repo)?;
        let blob = object.as_blob()
            .ok_or_else(|| anyhow::anyhow!("Object is not a blob"))?;

        String::from_utf8(blob.content().to_vec())
            .with_context(|| "Failed to convert blob content to UTF-8")
    }

    fn analyze_file_change(&self, file_path: &str, old_content: &str, new_content: &str) -> CodeChange {
        let line_changes = self.compute_line_changes(old_content, new_content);
        let complexity = self.compute_complexity_metrics(file_path, new_content);

        CodeChange {
            file_path: file_path.to_string(),
            old_content: old_content.to_string(),
            new_content: new_content.to_string(),
            line_changes,
            complexity,
        }
    }

    fn compute_line_changes(&self, old_content: &str, new_content: &str) -> Vec<LineChange> {
        let diff = TextDiff::configure()
            .algorithm(Algorithm::Patience)
            .diff_lines(old_content, new_content);

        let mut line_changes = Vec::new();
        let mut line_a_num = 1u32;
        let mut line_b_num = 1u32;

        for change in diff.iter_all_changes() {
            match change.tag() {
                similar::ChangeTag::Delete => {
                    line_changes.push(LineChange {
                        line_a: Some(line_a_num),
                        line_b: None,
                        change_type: ChangeType::Removed,
                        content: change.to_string().trim_end_matches('\n').to_string(),
                    });
                    line_a_num += 1;
                }
                similar::ChangeTag::Insert => {
                    line_changes.push(LineChange {
                        line_a: None,
                        line_b: Some(line_b_num),
                        change_type: ChangeType::Added,
                        content: change.to_string().trim_end_matches('\n').to_string(),
                    });
                    line_b_num += 1;
                }
                similar::ChangeTag::Equal => {
                    line_changes.push(LineChange {
                        line_a: Some(line_a_num),
                        line_b: Some(line_b_num),
                        change_type: ChangeType::Equal,
                        content: change.to_string().trim_end_matches('\n').to_string(),
                    });
                    line_a_num += 1;
                    line_b_num += 1;
                }
            }
        }

        line_changes
    }

    fn compute_complexity_metrics(&self, file_path: &str, content: &str) -> ComplexityMetrics {
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");


        if let Some(mut parser) = self.parsers.get_mut(extension) {
            println!("Using parser for extension: {}", extension);
            if let Some(tree) = parser.parse(content, None) {
                return self.analyze_syntax_tree(&tree, content.as_bytes());
            }
        }

        // Fallback to simple metrics
        ComplexityMetrics {
            lines_of_code: content.lines().count() as u32,
            ..Default::default()
        }
    }

    fn analyze_syntax_tree(&self, tree: &tree_sitter::Tree, source: &[u8]) -> ComplexityMetrics {
        let mut complexity = ComplexityMetrics::default();
        let root_node = tree.root_node();

        self.traverse_node(&root_node, source, &mut complexity, 0);
        self.loc_count(&root_node, &mut complexity);
        println!("Complexity metrics for syntax tree: {:?}", complexity);
        complexity
    }

    fn traverse_node(&self, node: &tree_sitter::Node, source: &[u8], complexity: &mut ComplexityMetrics, depth: u32) {
        complexity.max_nesting_depth = complexity.max_nesting_depth.max(depth);

        match node.kind() {
            "function_item" | "function_declaration" | "method_definition" => {
                complexity.function_count += 1;
            }
            "if_expression" | "if_statement" | "while_statement" | "for_statement" => {
                complexity.cyclomatic_complexity += 1;
            }
            _ => {}
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.traverse_node(&child, source, complexity, depth + 1);
            }
        }
    }

    fn loc_count(&self, node: &tree_sitter::Node, complexity: &mut ComplexityMetrics) {
        let mut lines = HashSet::new();

        fn visit(node: &tree_sitter::Node, lines: &mut HashSet<usize>) {
            if !node.is_named() || node.kind().contains("comment") {
                return;
            }
            
            lines.insert(node.start_position().row);
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    visit(&child, lines);
                }
            }
        }
        
        visit(node, &mut lines);
        complexity.lines_of_code = lines.len() as u32;
    }
}

impl Default for CodeAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyze Git diff and return JSON string
///
/// # Errors
///
/// Returns error if repository cannot be opened or commit cannot be found
pub fn analyze_git_diff_json(repo_path: &Path, commit_hash: &str, url: &str) -> Result<String> {
    let engine = CodeAnalysisEngine::new();
    let changes = engine.analyze_git_diff(repo_path, commit_hash, url)?;

    serde_json::to_string_pretty(&changes)
        .with_context(|| "Failed to serialize changes to JSON")
}

/// Creating External Function for Library Call from Another Service
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_diff(repo_path: *const c_char, commit_hash: *const c_char, url: *const c_char) -> *mut c_char {
    let unsafe_repo_path = unsafe { CStr::from_ptr(repo_path) }.to_str().unwrap_or("");
    let unsafe_commit_hash = unsafe { CStr::from_ptr(commit_hash) }.to_str().unwrap_or("");
    let unsafe_url = unsafe { CStr::from_ptr(url) }.to_str().unwrap_or("");

    let path = Path::new(unsafe_repo_path);
    let result = analyze_git_diff_json(path, unsafe_commit_hash, unsafe_url).unwrap_or_else(|msg| format!("Error: {msg}"));

    // Convert Rust String to CString and leak the pointer to C/Go
    CString::new(result).unwrap().into_raw()
}

/// To free the memory allocated above:
#[unsafe(no_mangle)]
pub extern "C" fn free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = unsafe { CString::from_raw(s) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_engine_creation() {
        let engine = CodeAnalysisEngine::new();
        assert_eq!(engine.config.max_concurrent_files, num_cpus::get());
    }

    #[test]
    fn test_should_analyze_file() {
        let engine = CodeAnalysisEngine::new();

        assert!(engine.should_analyze_file("main.rs"));
        assert!(engine.should_analyze_file("script.py"));
        assert!(engine.should_analyze_file("app.js"));
        assert!(!engine.should_analyze_file("image.png"));
        assert!(!engine.should_analyze_file("README.md"));
    }

    #[test]
    fn test_complexity_metrics() {
        let engine = CodeAnalysisEngine::new();
        let content = r#"
fn main() {
    if true {
        println!("Hello");
        if false {
            println!("World");
        }
    }
}
"#;

        let metrics = engine.compute_complexity_metrics("test.rs", content);
        assert!(metrics.lines_of_code > 0);
        assert!(metrics.function_count > 0);
        assert!(metrics.cyclomatic_complexity > 0);
    }
}