use std::fs;
use std::path::PathBuf;
use base64::prelude::*;
use eframe::egui;
use crate::ui::app::DiffAnalyzerApp;
use crate::ui::github;

pub fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|mut path| {
        path.push("git-diff-analyzer");
        path.push("config");
        path
    })
}

pub fn save_token(app: &DiffAnalyzerApp) {
    if let (Some(token), Some(config_path)) = (&app.github_token, get_config_path()) {
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        
        // Simple obfuscation (not cryptographically secure, but better than plain text)
        let encoded = BASE64_STANDARD.encode(token.as_bytes());
        let _ = fs::write(&config_path, encoded);
    }
}

pub fn load_token(app: &mut DiffAnalyzerApp) {
    if let Some(config_path) = get_config_path() {
        if let Ok(encoded) = fs::read_to_string(&config_path) {
            if let Ok(decoded_bytes) = BASE64_STANDARD.decode(&encoded) {
                if let Ok(token) = String::from_utf8(decoded_bytes) {
                    app.github_token = Some(token);
                }
            }
        }
    }
}

pub fn clear_token(app: &mut DiffAnalyzerApp) {
    app.github_token = None;
    if let Some(config_path) = get_config_path() {
        let _ = fs::remove_file(&config_path);
    }
}

pub fn render_auth_section(app: &mut DiffAnalyzerApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(35, 40, 48))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 65, 73)))
                .inner_margin(egui::Margin::same(32.0))
                .rounding(12.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("🔑").size(48.0));
                        ui.add_space(16.0);
                        ui.label(egui::RichText::new("Connect to GitHub").size(24.0).color(egui::Color32::WHITE));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Enter your Personal Access Token to get started").color(egui::Color32::from_rgb(139, 148, 158)));
                        ui.add_space(24.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Token:").color(egui::Color32::WHITE));
                            let response = ui.add_sized([300.0, 32.0], egui::TextEdit::singleline(&mut app.token_input)
                                .password(true)
                                .hint_text("ghp_xxxxxxxxxxxxxxxxxxxx"));
                            
                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                handle_token_submit(app);
                            }
                        });
                        
                        ui.add_space(16.0);
                        
                        let connect_btn = ui.add_sized([120.0, 36.0], egui::Button::new(
                            egui::RichText::new("Connect").size(14.0).color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(58, 113, 226)));
                        
                        if connect_btn.clicked() {
                            handle_token_submit(app);
                        }
                    });
                });
        });
    });
}

fn handle_token_submit(app: &mut DiffAnalyzerApp) {
    if app.token_input.trim().is_empty() {
        app.error = Some("Token cannot be empty".into());
    } else {
        app.github_token = Some(app.token_input.trim().to_string());
        if github::validate_token(app) {
            save_token(app);
            github::load_repos(app);
        } else {
            app.github_token = None;
            app.error = Some("Invalid GitHub token. Please check your token.".to_string());
        }
    }
}