use std::process::Command;

pub fn is_installed() -> bool {
    Command::new("gh").arg("--version").output().is_ok()
}

pub fn is_authenticated(host: &str) -> bool {
    Command::new("gh")
        .args(["auth", "status", "-h", host])
        .output()
        .is_ok_and(|out| out.status.success())
}

pub fn launch_login(host: &str) -> std::io::Result<bool> {
    Command::new("gh")
        .args(["auth", "login", "-h", host])
        .status()
        .map(|status| status.success())
}
