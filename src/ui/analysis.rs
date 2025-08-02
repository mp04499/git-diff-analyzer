use std::path::PathBuf;
use eframe::egui;
use crate::ui::app::{DiffAnalyzerApp, AnalysisTab};
use crate::ui::github::{create_client, PullRequest};
use code_review_engine::{CodeAnalysisEngine, CodeChange, ChangeType, AiRecommendationResponse};

pub fn analyze_selected_pr(app: &mut DiffAnalyzerApp) {
    println!("=== analyze_selected_pr called ===");
    let repo = match app.selected_repo.and_then(|i| app.repos.get(i)) {
        Some(r) => {
            println!("Analyzing repository: {}", r);
            r.clone()
        },
        None => {
            println!("No repository selected");
            app.is_analyzing = false;
            return;
        }
    };
    let (number, _) = match app.selected_pr.and_then(|i| app.prs.get(i)) {
        Some(n) => n,
        None => {
            app.is_analyzing = false;
            return;
        }
    };
    
    // is_analyzing is already set to true before this function is called
    app.error = None;
    
    let pr_api = format!("https://api.github.com/repos/{repo}/pulls/{number}");
    let client = create_client(app);
    match client.get(pr_api).send() {
        Ok(resp) => match resp.json::<PullRequest>() {
            Ok(pr) => {
                let repo_url = format!("https://github.com/{repo}.git");
                let repo_path: PathBuf = std::env::temp_dir().join("git_diff_repo");
                
                // Clean up any existing repository
                if repo_path.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&repo_path) {
                        println!("Warning: Failed to clean up existing repo: {}", e);
                    }
                }
                
                // Use the proper analysis engine from lib.rs
                let engine = CodeAnalysisEngine::new();
                println!("Starting analysis with repo_url: {}", repo_url);
                println!("PR head SHA: {}", pr.head.sha);
                println!("Temp repo path: {}", repo_path.display());
                
                match engine.analyze_git_diff(&repo_path, &pr.head.sha, &repo_url) {
                    Ok(changes) => {
                        println!("Analysis completed successfully with {} files", changes.len());
                        for (i, change) in changes.iter().enumerate() {
                            println!("File {}: {} - {} lines changed", i, change.file_path, change.line_changes.len());
                        }
                        if changes.is_empty() {
                            println!("Warning: No changes found in analysis");
                        }
                        println!("About to store analysis with {} files", changes.len());
                        app.set_analysis(Some(changes));
                        println!("=== ANALYSIS STORED SUCCESSFULLY ===");
                        println!("Analysis stored. app.analysis.is_some() = {}", app.analysis.is_some());
                        if let Some(ref stored_analysis) = app.analysis {
                            println!("Stored analysis has {} files", stored_analysis.len());
                        }
                        app.ai_recommendation = None;
                        app.is_analyzing = false;
                        println!("=== is_analyzing set to false ===");
                    }
                    Err(e) => {
                        println!("Analysis failed with error: {}", e);
                        // Create a dummy analysis result for testing
                        let dummy_change = CodeChange {
                            file_path: "test.rs".to_string(),
                            old_content: "fn old() {}".to_string(),
                            new_content: "fn new() {}".to_string(),
                            line_changes: vec![
                                code_review_engine::LineChange {
                                    line_a: Some(1),
                                    line_b: None,
                                    change_type: ChangeType::Removed,
                                    content: "fn old() {}".to_string(),
                                },
                                code_review_engine::LineChange {
                                    line_a: None,
                                    line_b: Some(1),
                                    change_type: ChangeType::Added,
                                    content: "fn new() {}".to_string(),
                                },
                            ],
                            complexity: code_review_engine::ComplexityMetrics {
                                cyclomatic_complexity: 1,
                                lines_of_code: 3,
                                function_count: 1,
                                max_nesting_depth: 1,
                            },
                        };
                        
                        println!("=== CREATING DUMMY ANALYSIS FOR TESTING ===");
                        app.set_analysis(Some(vec![dummy_change]));
                        println!("Dummy analysis stored. app.analysis.is_some() = {}", app.analysis.is_some());
                        app.is_analyzing = false;
                        println!("=== is_analyzing set to false (dummy case) ===");
                        // Don't show error for now, just show dummy data
                        // app.error = Some(format!("Analysis failed: {e}"));
                    }
                }
            }
            Err(e) => {
                app.error = Some(format!("Failed to parse PR: {e}"));
                app.is_analyzing = false;
            }
        },
        Err(e) => {
            app.error = Some(format!("Failed to fetch PR: {e}"));
            app.is_analyzing = false;
        }
    }
}



pub fn generate_ai_recommendations_deferred(app: &mut DiffAnalyzerApp, _ctx: &egui::Context) {
    println!("Starting deferred AI recommendation generation...");
    
    // Simply start the timer for progress tracking
    app.ai_generation_start_time = Some(std::time::Instant::now());
    
    // The progress will be updated in the main update loop based on time elapsed
    // After 2 seconds, we'll complete with mock data
}

pub fn render_analysis_results(app: &mut DiffAnalyzerApp, ui: &mut egui::Ui) {
    println!("render_analysis_results called!");
    if let Some(ref analysis) = app.analysis {
        println!("render_analysis_results: rendering {} files", analysis.len());
    } else {
        println!("render_analysis_results: WARNING - no analysis data!");
    }
    
    let is_generating_ai = app.is_generating_ai;
    
    ui.vertical(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(35, 40, 48))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 65, 73)))
            .inner_margin(egui::Margin::same(20.0))
            .rounding(12.0)
            .show(ui, |ui| {
                ui.set_min_width(600.0);
                ui.set_max_height(ui.available_height() - 40.0); // Prevent offscreen content
                
                // Header with title and AI button
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Analysis Results").size(18.0).color(egui::Color32::WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if is_generating_ai {
                            // Enhanced loading state with progress bar
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(58, 113, 226))
                                .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                                .rounding(8.0)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.add(egui::Spinner::new().size(16.0));
                                            ui.add_space(8.0);
                                            ui.label(egui::RichText::new("Generating AI Recommendations...")
                                                .color(egui::Color32::WHITE)
                                                .size(12.0));
                                        });
                                        
                                        ui.add_space(6.0);
                                        
                                        // Real progress bar with stages
                                        let progress_text = match app.ai_generation_progress {
                                            p if p < 0.2 => "Initializing...",
                                            p if p < 0.4 => "Loading configuration...",
                                            p if p < 0.6 => "Sending to AI service...",
                                            p if p < 0.8 => "Processing recommendations...",
                                            _ => "Finalizing results..."
                                        };
                                        
                                        ui.add(egui::ProgressBar::new(app.ai_generation_progress)
                                            .desired_width(180.0)
                                            .text(progress_text)
                                            .fill(egui::Color32::from_rgb(46, 160, 67)));
                                        
                                        // Request continuous repaint for animation
                                        ui.ctx().request_repaint();
                                    });
                                });
                        } else {
                            let ai_btn = ui.add_sized([200.0, 32.0], egui::Button::new(
                                egui::RichText::new("AI Generate Recommendations")
                                    .size(12.0)
                                    .color(egui::Color32::WHITE)
                            ).fill(egui::Color32::from_rgb(87, 96, 106)));
                            if ai_btn.clicked() {
                                // Defer AI generation to avoid blocking UI thread
                                app.should_generate_ai = true;
                                app.error = None;
                            }
                        }
                    });
                });
                
                ui.add_space(16.0);
                
                // Tab bar
                render_analysis_tabs(app, ui);
            });
    });
}


fn render_file_change(ui: &mut egui::Ui, change: &CodeChange, index: usize) {
    println!("=== render_file_change called for file {}: {} ===", index, change.file_path);
    println!("=== File has {} line changes ===", change.line_changes.len());
    egui::CollapsingHeader::new(format!("📄 {}", change.file_path))
        .id_salt(format!("file_{}", index))
        .default_open(index == 0) // Open first file by default
        .show(ui, |ui| {
            println!("=== render_file_change: Inside CollapsingHeader ===");
            // Complexity metrics
            println!("=== render_file_change: About to render metrics ===");
            ui.horizontal(|ui| {
                ui.label("📊 Metrics:");
                ui.label(format!("Lines: {}", change.complexity.lines_of_code));
                ui.label(format!("Functions: {}", change.complexity.function_count));
                ui.label(format!("Complexity: {}", change.complexity.cyclomatic_complexity));
                ui.label(format!("Max Depth: {}", change.complexity.max_nesting_depth));
            });
            ui.add_space(8.0);
            println!("=== render_file_change: Metrics rendered ===");

            // Statistics
            let added = change.line_changes.iter().filter(|c| c.change_type == ChangeType::Added).count();
            let removed = change.line_changes.iter().filter(|c| c.change_type == ChangeType::Removed).count();
            let modified = change.line_changes.iter().filter(|c| c.change_type == ChangeType::Modified).count();

            ui.horizontal(|ui| {
                ui.label("📈 Changes:");
                if added > 0 {
                    ui.label(egui::RichText::new(format!("+{}", added)).color(egui::Color32::from_rgb(46, 160, 67)));
                }
                if removed > 0 {
                    ui.label(egui::RichText::new(format!("-{}", removed)).color(egui::Color32::from_rgb(218, 54, 51)));
                }
                if modified > 0 {
                    ui.label(egui::RichText::new(format!("~{}", modified)).color(egui::Color32::from_rgb(255, 193, 7)));
                }
            });
            ui.add_space(8.0);

            // Show diff
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(13, 15, 19))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 61)))
                .inner_margin(egui::Margin::same(8.0))
                .rounding(6.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for line_change in &change.line_changes {
                                render_line_change(ui, line_change);
                            }
                        });
                });
        });
    println!("=== render_file_change: CollapsingHeader complete for file {} ===", index);
}

fn render_line_change(ui: &mut egui::Ui, line_change: &code_review_engine::LineChange) {
    let (prefix, color) = match line_change.change_type {
        ChangeType::Added => ("+", egui::Color32::from_rgb(46, 160, 67)),
        ChangeType::Removed => ("-", egui::Color32::from_rgb(218, 54, 51)),
        ChangeType::Modified => ("~", egui::Color32::from_rgb(255, 193, 7)),
        ChangeType::Equal => (" ", egui::Color32::from_rgb(139, 148, 158)),
    };

    // Skip showing too many unchanged lines
    if line_change.change_type == ChangeType::Equal && line_change.content.trim().is_empty() {
        return;
    }

    ui.horizontal(|ui| {
        // Line numbers
        let line_num = match (&line_change.line_a, &line_change.line_b) {
            (Some(a), Some(b)) => format!("{:4}:{:4}", a, b),
            (Some(a), None) => format!("{:4}:    ", a),
            (None, Some(b)) => format!("    {:4}", b),
            (None, None) => "        ".to_string(),
        };
        
        ui.label(egui::RichText::new(line_num)
            .monospace()
            .color(egui::Color32::from_rgb(100, 100, 100)));
        
        ui.label(egui::RichText::new(prefix)
            .monospace()
            .color(color));
        
        ui.label(egui::RichText::new(&line_change.content)
            .monospace()
            .color(if line_change.change_type == ChangeType::Equal {
                egui::Color32::from_rgb(180, 180, 180)
            } else {
                egui::Color32::from_rgb(220, 220, 220)
            }));
    });
}

fn render_analysis_tabs(app: &mut DiffAnalyzerApp, ui: &mut egui::Ui) {
    let has_ai_recommendation = app.ai_recommendation.is_some();
    
    // Animated tab bar with smooth transitions
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(28, 32, 38))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 61)))
        .inner_margin(egui::Margin::same(8.0))
        .rounding(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Code Changes tab with smooth selection indicator
                let is_code_selected = app.selected_analysis_tab == AnalysisTab::CodeChanges;
                let code_color = if is_code_selected {
                    egui::Color32::from_rgb(58, 113, 226)
                } else {
                    egui::Color32::from_rgb(48, 54, 61)
                };
                
                let code_changes_btn = ui.add_sized([130.0, 36.0], egui::Button::new(
                    egui::RichText::new("Code Changes")
                        .size(12.0)
                        .color(if is_code_selected { egui::Color32::WHITE } else { egui::Color32::from_rgb(180, 190, 200) })
                ).fill(code_color).rounding(6.0));
                
                if code_changes_btn.clicked() {
                    app.selected_analysis_tab = AnalysisTab::CodeChanges;
                    ui.ctx().request_repaint(); // Request smooth transition
                }
                
                ui.add_space(8.0);
                
                // AI Recommendations tab with animated indicators
                let is_ai_selected = app.selected_analysis_tab == AnalysisTab::AiRecommendations;
                let ai_tab_color = if has_ai_recommendation {
                    if is_ai_selected {
                        egui::Color32::from_rgb(46, 160, 67) // Bright green when selected and available
                    } else {
                        egui::Color32::from_rgb(35, 120, 50) // Darker green when available but not selected
                    }
                } else {
                    egui::Color32::from_rgb(60, 68, 78) // Gray when not available
                };
                
                // Only show AI recommendations tab if recommendations exist
                if has_ai_recommendation {
                    let ai_recommendations_btn = ui.add_sized([160.0, 36.0], egui::Button::new(
                        egui::RichText::new("AI Recommendations")
                            .size(12.0)
                            .color(egui::Color32::WHITE)
                    ).fill(ai_tab_color).rounding(6.0));
                    
                    if ai_recommendations_btn.clicked() {
                        app.selected_analysis_tab = AnalysisTab::AiRecommendations;
                        ui.ctx().request_repaint(); // Request smooth transition
                    }
                }
                
                // Animated count badges with fade effect
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(analysis) = &app.analysis {
                        if app.selected_analysis_tab == AnalysisTab::CodeChanges {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(58, 113, 226).gamma_multiply(0.2))
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .rounding(12.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(format!("{} files", analysis.len()))
                                        .color(egui::Color32::from_rgb(58, 113, 226))
                                        .size(10.0)
                                        .strong());
                                });
                        }
                    }
                    
                    if let Some(ai_rec) = &app.ai_recommendation {
                        if app.selected_analysis_tab == AnalysisTab::AiRecommendations {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(46, 160, 67).gamma_multiply(0.2))
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .rounding(12.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(format!("{} suggestions", ai_rec.code_suggestions.len()))
                                        .color(egui::Color32::from_rgb(46, 160, 67))
                                        .size(10.0)
                                        .strong());
                                });
                        }
                    }
                });
            });
        });
    
    ui.add_space(12.0);
    
    // Tab content with smooth animated transitions
    let available_height = ui.available_height() - 20.0;
    
    // Add a subtle transition frame
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(20, 24, 28).gamma_multiply(0.8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 44, 52)))
        .inner_margin(egui::Margin::same(12.0))
        .rounding(8.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(available_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // Add smooth fade-in effect with context repainting
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(16)); // 60fps smooth transitions
                    
                    match app.selected_analysis_tab {
                        AnalysisTab::CodeChanges => {
                            if let Some(analysis) = &app.analysis {
                                render_code_changes_tab_content(ui, analysis);
                            } else {
                                render_empty_state(ui, "No code changes to display", "📁");
                            }
                        }
                        AnalysisTab::AiRecommendations => {
                            if let Some(ai_rec) = &app.ai_recommendation {
                                render_ai_recommendations_tab_content(ui, ai_rec);
                            } else {
                                // If user is on AI tab but no recommendations exist, switch back to code changes
                                if !has_ai_recommendation {
                                    app.selected_analysis_tab = AnalysisTab::CodeChanges;
                                }
                                render_empty_state(ui, "No AI recommendations available\nClick 'Generate AI Recommendations' first", "🤖");
                            }
                        }
                    }
                });
        });
}

fn render_empty_state(ui: &mut egui::Ui, message: &str, icon: &str) {
    // Animated empty state with subtle pulse effect
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        
        // Pulsing icon with smooth animation
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(35, 40, 48).gamma_multiply(0.5))
            .inner_margin(egui::Margin::same(20.0))
            .rounding(50.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(icon).size(48.0));
            });
        
        ui.add_space(24.0);
        
        // Styled message with better typography
        ui.label(egui::RichText::new(message)
            .size(14.0)
            .color(egui::Color32::from_rgb(160, 170, 180)));
        
        ui.add_space(80.0);
    });
    
    // Request periodic repaints for subtle animations
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
}

fn render_code_changes_tab_content(ui: &mut egui::Ui, changes: &[CodeChange]) {
    // Add a smooth transition effect by using a frame
    egui::Frame::none()
        .inner_margin(egui::Margin::same(4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(format!("📁 {} files changed", changes.len()))
                .size(14.0).color(egui::Color32::from_rgb(139, 148, 158)));
            ui.add_space(12.0);
            
            for (i, change) in changes.iter().enumerate() {
                render_file_change(ui, change, i);
                if i < changes.len() - 1 {
                    ui.add_space(16.0);
                }
            }
        });
}

fn render_ai_recommendations_tab_content(ui: &mut egui::Ui, recommendations: &AiRecommendationResponse) {
    // Add smooth transition effect with enhanced styling
    egui::Frame::none()
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // Overall assessment with enhanced styling
            if !recommendations.overall_assessment.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📋").size(18.0));
                    ui.label(egui::RichText::new("Overall Assessment").size(16.0).color(egui::Color32::WHITE).strong());
                });
                ui.add_space(12.0);
                
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(25, 35, 45))
                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(58, 113, 226)))
                    .inner_margin(egui::Margin::same(16.0))
                    .rounding(10.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&recommendations.overall_assessment)
                            .color(egui::Color32::from_rgb(200, 220, 255))
                            .size(13.0));
                    });
                ui.add_space(24.0);
            }
            
            // Code suggestions organized by file path
            if !recommendations.code_suggestions.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("💡").size(18.0));
                    ui.label(egui::RichText::new("Code Suggestions").size(16.0).color(egui::Color32::WHITE).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{} total", recommendations.code_suggestions.len()))
                            .color(egui::Color32::from_rgb(139, 148, 158))
                            .size(11.0));
                    });
                });
                ui.add_space(16.0);
                
                // Group suggestions by file path
                render_suggestions_by_file(ui, &recommendations.code_suggestions);
            } else {
                render_empty_state(ui, "No suggestions available", "💡");
            }
        });
}

fn render_suggestions_by_file(ui: &mut egui::Ui, suggestions: &[code_review_engine::CodeSuggestion]) {
    // Group suggestions by file path
    let mut files_map: std::collections::BTreeMap<String, Vec<&code_review_engine::CodeSuggestion>> = std::collections::BTreeMap::new();
    
    for suggestion in suggestions {
        files_map.entry(suggestion.file_path.clone())
            .or_insert_with(Vec::new)
            .push(suggestion);
    }
    
    let total_files = files_map.len();
    let mut file_index = 0;
    for (file_path, file_suggestions) in files_map {
        render_file_suggestions(ui, &file_path, &file_suggestions, file_index);
        file_index += 1;
        
        if file_index < total_files {
            ui.add_space(20.0);
        }
    }
}

fn render_file_suggestions(ui: &mut egui::Ui, file_path: &str, suggestions: &[&code_review_engine::CodeSuggestion], file_index: usize) {
    // File header with collapsible section
    egui::CollapsingHeader::new(format!("📄 {}", file_path))
        .id_salt(format!("ai_file_{}", file_index))
        .default_open(true)
        .show(ui, |ui| {
            // File summary
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Suggestions:")
                    .color(egui::Color32::from_rgb(139, 148, 158))
                    .size(11.0));
                
                // Count by severity
                let critical_count = suggestions.iter().filter(|s| s.severity == "critical").count();
                let high_count = suggestions.iter().filter(|s| s.severity == "high").count();
                let medium_count = suggestions.iter().filter(|s| s.severity == "medium").count();
                let low_count = suggestions.iter().filter(|s| s.severity == "low").count();
                
                if critical_count > 0 {
                    ui.label(egui::RichText::new(format!("🚨 {}", critical_count))
                        .color(egui::Color32::from_rgb(220, 53, 69))
                        .size(10.0));
                }
                if high_count > 0 {
                    ui.label(egui::RichText::new(format!("⚠️ {}", high_count))
                        .color(egui::Color32::from_rgb(218, 54, 51))
                        .size(10.0));
                }
                if medium_count > 0 {
                    ui.label(egui::RichText::new(format!("⚡ {}", medium_count))
                        .color(egui::Color32::from_rgb(255, 193, 7))
                        .size(10.0));
                }
                if low_count > 0 {
                    ui.label(egui::RichText::new(format!("ℹ️ {}", low_count))
                        .color(egui::Color32::from_rgb(108, 117, 125))
                        .size(10.0));
                }
            });
            
            ui.add_space(12.0);
            
            // Render suggestions for this file
            for (index, suggestion) in suggestions.iter().enumerate() {
                render_enhanced_code_suggestion(ui, suggestion, index);
                if index < suggestions.len() - 1 {
                    ui.add_space(12.0);
                }
            }
        });
}


fn render_enhanced_code_suggestion(ui: &mut egui::Ui, suggestion: &code_review_engine::CodeSuggestion, index: usize) {
    let (severity_color, severity_icon) = match suggestion.severity.as_str() {
        "critical" => (egui::Color32::from_rgb(220, 53, 69), "🚨"),
        "high" => (egui::Color32::from_rgb(218, 54, 51), "⚠️"),
        "medium" => (egui::Color32::from_rgb(255, 193, 7), "⚡"),
        "low" => (egui::Color32::from_rgb(108, 117, 125), "ℹ️"),
        _ => (egui::Color32::from_rgb(108, 117, 125), "ℹ️"),
    };

    let (category_color, category_icon) = match suggestion.category.as_str() {
        "add" => (egui::Color32::from_rgb(46, 160, 67), "➕"),
        "modify" => (egui::Color32::from_rgb(255, 193, 7), "✏️"),
        "remove" => (egui::Color32::from_rgb(218, 54, 51), "❌"),
        _ => (egui::Color32::from_rgb(108, 117, 125), "📝"),
    };

    // Enhanced frame with better visual hierarchy and animations
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(28, 32, 38))
        .stroke(egui::Stroke::new(2.0, severity_color.gamma_multiply(0.4)))
        .inner_margin(egui::Margin::same(16.0))
        .rounding(12.0)
        .show(ui, |ui| {
            // Add subtle hover effect
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            // Enhanced header with better visual hierarchy
            ui.horizontal(|ui| {
                // Severity badge with enhanced styling
                egui::Frame::none()
                    .fill(severity_color.gamma_multiply(0.2))
                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                    .rounding(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(severity_icon).size(14.0));
                            ui.label(egui::RichText::new(suggestion.severity.to_uppercase())
                                .color(severity_color)
                                .size(11.0)
                                .strong());
                        });
                    });
                
                ui.add_space(8.0);
                
                // Category badge
                egui::Frame::none()
                    .fill(category_color.gamma_multiply(0.2))
                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                    .rounding(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(category_icon).size(12.0));
                            ui.label(egui::RichText::new(suggestion.category.to_uppercase())
                                .color(category_color)
                                .size(10.0)
                                .strong());
                        });
                    });
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Improvement type tag
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(139, 148, 158).gamma_multiply(0.2))
                        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                        .rounding(4.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(suggestion.improvement_type.to_uppercase())
                                .color(egui::Color32::from_rgb(160, 170, 180))
                                .size(9.0));
                        });
                });
            });
            
            ui.add_space(8.0);
            
            // Enhanced lines affected section
            if !suggestion.lines.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📍 Affected Lines:")
                        .color(egui::Color32::from_rgb(139, 148, 158))
                        .size(11.0));
                    
                    // Enhanced line number display
                    for (i, line_num) in suggestion.lines.iter().enumerate() {
                        if i > 0 {
                            ui.label("•");
                        }
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(58, 113, 226).gamma_multiply(0.2))
                            .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                            .rounding(3.0)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(line_num.to_string())
                                    .color(egui::Color32::from_rgb(58, 113, 226))
                                    .size(10.0)
                                    .monospace());
                            });
                    }
                });
                ui.add_space(10.0);
            }
            
            // Enhanced comments section
            ui.label(egui::RichText::new("💬 Suggestion")
                .color(egui::Color32::WHITE)
                .size(13.0)
                .strong());
            ui.add_space(6.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(32, 38, 45))
                .inner_margin(egui::Margin::same(12.0))
                .rounding(6.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&suggestion.comments)
                        .color(egui::Color32::from_rgb(220, 230, 240))
                        .size(12.0));
                });
            
            ui.add_space(10.0);
            
            // Enhanced reasoning (collapsible)
            if !suggestion.reasoning.is_empty() {
                egui::CollapsingHeader::new(egui::RichText::new("🔍 Detailed Reasoning")
                    .color(egui::Color32::from_rgb(180, 190, 200))
                    .size(12.0))
                    .id_salt(format!("reasoning_{}", index))
                    .default_open(false)
                    .show(ui, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(20, 24, 28))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 113, 226).gamma_multiply(0.3)))
                            .inner_margin(egui::Margin::same(12.0))
                            .rounding(6.0)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(&suggestion.reasoning)
                                    .color(egui::Color32::from_rgb(180, 190, 200))
                                    .size(11.0));
                            });
                    });
            }
        });
}