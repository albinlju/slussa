use tokio::task;

use crate::{app::App, providers::github, tui::Action};

impl App {
    pub(super) fn spawn_load_prs(&self) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(github::fetch_prs)
                .await
                .unwrap_or_else(|join_err| Err(github::FetchError::ParseFailed(
                    format!("worker thread panicked: {join_err}"),
                )))
                .map_err(|e| e.to_string());
            tx.send(Action::PrsLoaded(result)).ok();
        });
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(move || github::fetch_commits(pr_id))
                .await
                .unwrap_or_else(|join_err| Err(github::FetchError::ParseFailed(
                    format!("worker thread panicked: {join_err}"),
                )))
                .map_err(|e| e.to_string());
            tx.send(Action::CommitsLoaded(pr_id, result)).ok();
        });
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(move || github::fetch_diff(pr_id))
                .await
                .unwrap_or_else(|join_err| Err(github::FetchError::ParseFailed(
                    format!("worker thread panicked: {join_err}"),
                )))
                .map_err(|e| e.to_string());
            tx.send(Action::DiffLoaded(pr_id, result)).ok();
        });
    }

    pub(super) fn spawn_load_comments(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(move || github::fetch_comments(pr_id))
                .await
                .unwrap_or_else(|join_err| Err(github::FetchError::ParseFailed(
                    format!("worker thread panicked: {join_err}"),
                )))
                .map_err(|e| e.to_string());
            tx.send(Action::CommentsLoaded(pr_id, result)).ok();
        });
    }

    pub(super) fn spawn_load_review_threads(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let result = task::spawn_blocking(move || github::fetch_review_threads(pr_id))
                .await
                .unwrap_or_else(|join_err| Err(github::FetchError::ParseFailed(
                    format!("worker thread panicked: {join_err}"),
                )))
                .map_err(|e| e.to_string());
            tx.send(Action::ReviewThreadsLoaded(pr_id, result)).ok();
        });
    }
}
