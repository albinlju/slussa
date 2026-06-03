use crate::domain::pr::PullRequest;
use std::process::Command;

pub fn fetch_prs() -> Vec<PullRequest> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--json",
            "number,title,author,state,headRefName,baseRefName",
        ])
        .output()
        .expect("gh not installed");

    let _json = String::from_utf8_lossy(&output.stdout);

    // senare: serde parsing
    vec![]
}
