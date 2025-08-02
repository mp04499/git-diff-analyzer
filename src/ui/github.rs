use reqwest::blocking::Client;
use reqwest::header;
use reqwest::header::HeaderValue;
use serde::Deserialize;
use eframe::egui;
use crate::ui::app::DiffAnalyzerApp;

#[derive(Deserialize)]
struct Repo {
    full_name: String,
}

#[derive(Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub head: PullRequestHead,
}

#[derive(Deserialize)]
pub struct PullRequestHead {
    pub sha: String,
}

pub fn create_client(app: &DiffAnalyzerApp) -> Client {
    let mut headers = header::HeaderMap::new();
    let builder = Client::builder();
    if let Some(token) = &app.github_token {
        let value = HeaderValue::from_str(&format!("Bearer {token}", token=token));
        headers.insert(header::AUTHORIZATION, value.unwrap());
        builder
            .user_agent("diff-analyzer-ui")
            .default_headers(headers)
            .build()
            .unwrap()
    } else {
        builder.user_agent("diff-analyzer-ui").build().unwrap()
    }
}

pub fn validate_token(app: &DiffAnalyzerApp) -> bool {
    if let Some(_token) = &app.github_token {
        let client = create_client(app);
        match client.get("https://api.github.com/user").send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    } else {
        false
    }
}

pub fn load_repos(app: &mut DiffAnalyzerApp) {
    app.is_loading_repos = true;
    app.error = None;
    app.repos.clear();
    
    let client = create_client(app);
    
    // Load user's own repositories with pagination
    let mut page = 1;
    loop {
        let url = format!("https://api.github.com/user/repos?per_page=100&page={}&sort=updated", page);
        match client.get(&url).send() {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Vec<Repo>>() {
                        Ok(list) => {
                            if list.is_empty() {
                                break; // No more pages
                            }
                            app.repos.extend(list.into_iter().map(|r| r.full_name));
                            page += 1;
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to parse repos page {}: {}", page, e));
                            app.is_loading_repos = false;
                            return;
                        }
                    }
                } else {
                    app.error = Some(format!("GitHub API error: {}", resp.status()));
                    app.is_loading_repos = false;
                    return;
                }
            }
            Err(e) => {
                app.error = Some(format!("Failed to fetch repos page {}: {}", page, e));
                app.is_loading_repos = false;
                return;
            }
        }
    }
    
    // Also load repositories from organizations the user belongs to
    match client.get("https://api.github.com/user/orgs").send() {
        Ok(resp) => {
            if resp.status().is_success() {
                if let Ok(orgs) = resp.json::<Vec<serde_json::Value>>() {
                    for org in orgs {
                        if let Some(org_login) = org["login"].as_str() {
                            // Load repos for this organization
                            let mut org_page = 1;
                            loop {
                                let org_url = format!("https://api.github.com/orgs/{}/repos?per_page=100&page={}&sort=updated", org_login, org_page);
                                match client.get(&org_url).send() {
                                    Ok(org_resp) => {
                                        if org_resp.status().is_success() {
                                            match org_resp.json::<Vec<Repo>>() {
                                                Ok(org_list) => {
                                                    if org_list.is_empty() {
                                                        break; // No more pages for this org
                                                    }
                                                    app.repos.extend(org_list.into_iter().map(|r| r.full_name));
                                                    org_page += 1;
                                                }
                                                Err(_) => break, // Skip this org on parse error
                                            }
                                        } else {
                                            break; // Skip this org on API error
                                        }
                                    }
                                    Err(_) => break, // Skip this org on network error
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            // Ignore org loading errors, user repos should still work
        }
    }
    
    // Sort repositories alphabetically for better UX
    app.repos.sort();
    
    app.is_loading_repos = false;
    println!("Loaded {} repositories", app.repos.len()); // Debug output
}

pub fn load_prs(app: &mut DiffAnalyzerApp) {
    if let Some(repo) = app.selected_repo.and_then(|i| app.repos.get(i)) {
        // is_loading_prs is already set to true before this function is called
        app.error = None;
        app.prs.clear(); // Clear existing PRs
        
        println!("Loading PRs for repository: {}", repo); // Debug output
        
        let url = format!("https://api.github.com/repos/{repo}/pulls?state=open&per_page=100");
        let client = create_client(app);
        match client.get(&url).send() {
            Ok(resp) => {
                println!("PR API response status: {}", resp.status()); // Debug output
                if resp.status().is_success() {
                    match resp.json::<Vec<PullRequest>>() {
                        Ok(list) => {
                            println!("Successfully loaded {} PRs", list.len()); // Debug output
                            app.prs = list.into_iter().map(|p| (p.number, p.title)).collect();
                            app.is_loading_prs = false;
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to parse PRs: {e}"));
                            app.is_loading_prs = false;
                            println!("Failed to parse PRs: {}", e); // Debug output
                        }
                    }
                } else {
                    let status = resp.status();
                    let error_text = resp.text().unwrap_or_default();
                    app.error = Some(format!("GitHub API error: {} - {}", status, error_text));
                    app.is_loading_prs = false;
                    println!("GitHub API error: {}", status); // Debug output
                }
            },
            Err(e) => {
                app.error = Some(format!("Failed to fetch PRs: {e}"));
                app.is_loading_prs = false;
                println!("Failed to fetch PRs: {}", e); // Debug output
            }
        }
    } else {
        println!("No repository selected for PR loading"); // Debug output
        app.is_loading_prs = false;
    }
}

pub fn render_repo_selection(app: &mut DiffAnalyzerApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(35, 40, 48))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 65, 73)))
            .inner_margin(egui::Margin::same(20.0))
            .rounding(12.0)
            .show(ui, |ui| {
                ui.set_min_width(400.0);
                ui.label(egui::RichText::new("📚 Repository Selection").size(18.0).color(egui::Color32::WHITE));
                ui.add_space(16.0);
                
                println!("UI State: repos={}, selected_repo={:?}, prs={}, selected_pr={:?}", 
                    app.repos.len(), app.selected_repo, app.prs.len(), app.selected_pr);
                
                if app.repos.is_empty() {
                    if app.is_loading_repos {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label("Loading repositories...");
                        });
                    } else {
                        let reload_btn = ui.add_sized([180.0, 32.0], egui::Button::new("🔄 Reload Repositories"));
                        if reload_btn.clicked() {
                            load_repos(app);
                        }
                    }
                } else {
                    ui.label(egui::RichText::new("Repository:").color(egui::Color32::WHITE));
                    ui.add_space(4.0);
                    
                    // Search box for repositories
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        ui.add_sized([300.0, 24.0], egui::TextEdit::singleline(&mut app.repo_search)
                            .hint_text("Search repositories..."));
                    });
                    ui.add_space(8.0);
                    
                    // Filter repositories based on search
                    let filtered_repos: Vec<(usize, String)> = app.repos.iter().enumerate()
                        .filter(|(_, repo)| {
                            if app.repo_search.is_empty() {
                                true
                            } else {
                                repo.to_lowercase().contains(&app.repo_search.to_lowercase())
                            }
                        })
                        .map(|(idx, repo)| (idx, repo.clone()))
                        .collect();
                    
                    // Show count of repositories
                    ui.label(egui::RichText::new(format!("Showing {} of {} repositories", filtered_repos.len(), app.repos.len()))
                        .color(egui::Color32::from_rgb(139, 148, 158)));
                    ui.add_space(4.0);
                        
                    egui::ComboBox::from_id_salt("repo_select")
                        .selected_text(
                            app.selected_repo
                                .and_then(|i| app.repos.get(i))
                                .cloned()
                                .unwrap_or_else(|| "Select a repository...".to_string()),
                        )
                        .width(360.0)
                        .height(300.0) // Make dropdown taller to show more repos
                        .show_ui(ui, |ui| {
                            for (idx, repo) in filtered_repos.iter().take(50) { // Limit to 50 for performance
                                let was_selected = app.selected_repo == Some(*idx);
                                if ui.selectable_value(&mut app.selected_repo, Some(*idx), repo).clicked() && !was_selected {
                                    // Repository changed, reset PR state
                                    println!("=== REPOSITORY CHANGED - CLEARING ANALYSIS ===");
                                    app.prs.clear();
                                    app.selected_pr = None;
                                    app.set_analysis(None);
                                    app.ai_recommendation = None;
                                }
                            }
                            if filtered_repos.len() > 50 {
                                ui.label(egui::RichText::new(format!("... and {} more", filtered_repos.len() - 50))
                                    .color(egui::Color32::from_rgb(139, 148, 158)));
                            }
                        });

                    if let Some(_repo_idx) = app.selected_repo {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("Pull Request:").color(egui::Color32::WHITE));
                        ui.add_space(4.0);
                        
                        if app.is_loading_prs {
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new());
                                ui.label("Loading pull requests...");
                            });
                        } else if app.prs.is_empty() {
                            ui.horizontal(|ui| {
                                let load_prs_btn = ui.add_sized([140.0, 32.0], egui::Button::new("📥 Load PRs"));
                                if load_prs_btn.clicked() {
                                    app.should_load_prs = true;
                                }
                                ui.label(egui::RichText::new("No open pull requests found")
                                    .color(egui::Color32::from_rgb(139, 148, 158)));
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{} open pull requests", app.prs.len()))
                                    .color(egui::Color32::from_rgb(139, 148, 158)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let reload_prs_btn = ui.add_sized([80.0, 24.0], egui::Button::new("🔄"));
                                    if reload_prs_btn.clicked() {
                                        app.should_load_prs = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            egui::ComboBox::from_id_salt("pr_select")
                                .selected_text(
                                    app.selected_pr
                                        .and_then(|i| app.prs.get(i).map(|p| format!("#{} {}", p.0, p.1)))
                                        .unwrap_or_else(|| "Select a pull request...".to_string()),
                                )
                                .width(360.0)
                                .height(300.0)
                                .show_ui(ui, |ui| {
                                    for (idx, (number, title)) in app.prs.iter().enumerate() {
                                        ui.selectable_value(&mut app.selected_pr, Some(idx), format!("#{} {}", number, title));
                                    }
                                });

                            if app.selected_pr.is_some() {
                                println!("UI: PR selected (index: {:?}), showing analyze section", app.selected_pr);
                                ui.add_space(16.0);
                                if app.is_analyzing {
                                    println!("UI: Currently analyzing, showing spinner");
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Spinner::new());
                                        ui.label("Analyzing...");
                                    });
                                } else {
                                    println!("UI: Rendering analyze button");
                                    let analyze_btn = ui.add_sized([140.0, 36.0], egui::Button::new(
                                        egui::RichText::new("🔍 Analyze").size(14.0).color(egui::Color32::WHITE)
                                    ).fill(egui::Color32::from_rgb(58, 113, 226)));
                                    if analyze_btn.clicked() {
                                        println!("=== ANALYZE BUTTON CLICKED ===");
                                        println!("Setting should_analyze_pr = true");
                                        app.should_analyze_pr = true;
                                        println!("should_analyze_pr is now: {}", app.should_analyze_pr);
                                    }
                                }
                            } else {
                                println!("UI: No PR selected, analyze button not shown");
                            }
                        }
                    }
                }
            });
    });
}