use std::process;
fn main() {
    if let Some((rev, branch)) = git_version() {
        println!("cargo:rustc-env=SHAKEN_GIT_REV={}", rev);
        println!("cargo:rustc-env=SHAKEN_GIT_BRANCH={}", branch);
    }
}

fn git_version() -> Option<(String, String)> {
    fn do_git(args: &[&str]) -> Option<String> {
        let git = process::Command::new("git").args(args).output();
        git.ok().and_then(|out| {
            let res = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if res.is_empty() {
                None
            } else {
                Some(res)
            }
        })
    }

    let branch = option_env!("SHAKEN_BRANCH").unwrap_or_else(|| "dev");
    do_git(&["rev-parse", "--short=12", branch]).and_then(|rev| {
        do_git(&["rev-parse", "--abbrev-ref", branch]).and_then(|branch| Some((rev, branch)))
    })
}
