use tokio::task;

use crate::{app::App, providers::github, tui::Action};

impl App {
    pub(super) fn spawn_load_prs(&self) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let prs = task::spawn_blocking(github::fetch_prs)
                .await
                .unwrap_or_default();
            tx.send(Action::PrsLoaded(prs)).ok();
        });
    }

    pub(super) fn spawn_load_commits(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let commits = task::spawn_blocking(move || github::fetch_commits(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::CommitsLoaded(pr_id, commits)).ok();
        });
    }

    pub(super) fn spawn_load_diff(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let diff = task::spawn_blocking(move || github::fetch_diff(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::DiffLoaded(pr_id, diff)).ok();
        });
    }

    pub(super) fn spawn_load_comments(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let comments = task::spawn_blocking(move || github::fetch_comments(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::CommentsLoaded(pr_id, comments)).ok();
        });
    }

    pub(super) fn spawn_load_review_threads(&self, pr_id: u64) {
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let threads = task::spawn_blocking(move || github::fetch_review_threads(pr_id))
                .await
                .unwrap_or_default();
            tx.send(Action::ReviewThreadsLoaded(pr_id, threads)).ok();
        });
    }
}
