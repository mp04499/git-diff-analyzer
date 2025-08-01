//! Graphical UI for analyzing GitHub pull requests.

use code_review_engine::analyze_git_diff_json;
use eframe::{App, NativeOptions, egui};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Default)]
struct DiffAnalyzerApp {
    github_token: Option<String>,
    token_input: String,
    repos: Vec<String>,
    selected_repo: Option<usize>,
    prs: Vec<(u32, String)>,
    selected_pr: Option<usize>,
    analysis: Option<String>,
    ai_recommendation: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Repo {
    full_name: String,
}

#[derive(Deserialize)]
struct PullRequest {
    number: u32,
    title: String,
    head: PullRequestHead,
}

#[derive(Deserialize)]
struct PullRequestHead {
    sha: String,
}

impl DiffAnalyzerApp {
    fn client(&self) -> Client {
        let builder = Client::builder();
        if let Some(token) = &self.github_token {
            builder
                .user_agent("diff-analyzer-ui")
                .bearer_auth(token)
                .build()
                .unwrap()
        } else {
            builder.user_agent("diff-analyzer-ui").build().unwrap()
        }
    }

    fn load_repos(&mut self) {
        let client = self.client();
        match client.get("https://api.github.com/user/repos").send() {
            Ok(resp) => match resp.json::<Vec<Repo>>() {
                Ok(list) => {
                    self.repos = list.into_iter().map(|r| r.full_name).collect();
                }
                Err(e) => self.error = Some(format!("Failed to parse repos: {e}")),
            },
            Err(e) => self.error = Some(format!("Failed to fetch repos: {e}")),
        }
    }

    fn load_prs(&mut self) {
        if let Some(repo) = self.selected_repo.and_then(|i| self.repos.get(i)) {
            let url = format!("https://api.github.com/repos/{repo}/pulls");
            let client = self.client();
            match client.get(url).send() {
                Ok(resp) => match resp.json::<Vec<PullRequest>>() {
                    Ok(list) => {
                        self.prs = list.into_iter().map(|p| (p.number, p.title)).collect();
                    }
                    Err(e) => self.error = Some(format!("Failed to parse PRs: {e}")),
                },
                Err(e) => self.error = Some(format!("Failed to fetch PRs: {e}")),
            }
        }
    }

    fn analyze_selected_pr(&mut self) {
        let repo = match self.selected_repo.and_then(|i| self.repos.get(i)) {
            Some(r) => r.clone(),
            None => return,
        };
        let (number, _) = match self.selected_pr.and_then(|i| self.prs.get(i)) {
            Some(n) => *n,
            None => return,
        };
        let pr_api = format!("https://api.github.com/repos/{repo}/pulls/{number}");
        let client = self.client();
        match client.get(pr_api).send() {
            Ok(resp) => match resp.json::<PullRequest>() {
                Ok(pr) => {
                    let repo_url = format!("https://github.com/{repo}.git");
                    let repo_path: PathBuf = std::env::temp_dir().join("git_diff_repo");
                    match analyze_git_diff_json(&repo_path, &pr.head.sha, &repo_url) {
                        Ok(json) => {
                            self.analysis = Some(json);
                            self.ai_recommendation = None;
                        }
                        Err(e) => self.error = Some(format!("Analysis failed: {e}")),
                    }
                }
                Err(e) => self.error = Some(format!("Failed to parse PR: {e}")),
            },
            Err(e) => self.error = Some(format!("Failed to fetch PR: {e}")),
        }
    }

    fn generate_ai(&mut self) {
        if self.analysis.is_some() {
            self.ai_recommendation = Some("AI recommendations not implemented".to_string());
        }
    }
}

impl App for DiffAnalyzerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Git Diff Analyzer");
            if let Some(err) = self.error.take() {
                ui.colored_label(egui::Color32::RED, err);
            }

            if self.github_token.is_none() {
                ui.label("GitHub Personal Access Token:");
                ui.text_edit_singleline(&mut self.token_input);
                if ui.button("Connect").clicked() {
                    if self.token_input.trim().is_empty() {
                        self.error = Some("Token cannot be empty".into());
                    } else {
                        self.github_token = Some(self.token_input.trim().to_string());
                        self.load_repos();
                    }
                }
                return;
            }

            if self.repos.is_empty() {
                if ui.button("Reload Repositories").clicked() {
                    self.load_repos();
                }
                return;
            }

            ui.label("Select Repository:");
            egui::ComboBox::from_id_source("repo_select")
                .selected_text(
                    self.selected_repo
                        .and_then(|i| self.repos.get(i))
                        .cloned()
                        .unwrap_or_default(),
                )
                .show_ui(ui, |ui| {
                    for (idx, repo) in self.repos.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_repo, Some(idx), repo);
                    }
                });

            if let Some(_repo_idx) = self.selected_repo {
                if self.prs.is_empty() && ui.button("Load Pull Requests").clicked() {
                    self.load_prs();
                }

                if !self.prs.is_empty() {
                    ui.label("Select Pull Request:");
                    egui::ComboBox::from_id_source("pr_select")
                        .selected_text(
                            self.selected_pr
                                .and_then(|i| self.prs.get(i).map(|p| &p.1))
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (idx, pr) in self.prs.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_pr, Some(idx), &pr.1);
                            }
                        });

                    if self.selected_pr.is_some() && ui.button("Analyze").clicked() {
                        self.analyze_selected_pr();
                    }
                }
            }

            if let Some(analysis) = &mut self.analysis {
                ui.separator();
                ui.label("Analysis:");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.text_edit_multiline(analysis);
                });
                if ui.button("Generate AI Recommendations").clicked() {
                    self.generate_ai();
                }
                if let Some(ai) = &self.ai_recommendation {
                    ui.separator();
                    ui.label("AI Recommendations:");
                    ui.label(ai);
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions::default();
    eframe::run_native(
        "Diff Analyzer UI",
        options,
        Box::new(|_cc| Box::<DiffAnalyzerApp>::default()),
    )
}
