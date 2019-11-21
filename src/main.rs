use colored::*;
use git2::BranchType;
use git2::Repository;
use reqwest;
use serde_derive::Deserialize;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    /// Circleci token
    ///
    /// This argument is optional, if not provided it will look for a CIRCLECI_TOKEN environment
    /// variable
    #[structopt(long = "token", short = "t")]
    token: Option<String>,
}

type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::from_args();

    let token = if let Some(token) = args.token {
        token
    } else {
        use std::env;
        env::var("CIRCLECI_TOKEN")
            .expect("Missing --token argument, or CIRCLECI_TOKEN environment variable")
    };

    let url = format!(
        "https://circleci.com/api/v1.1/recent-builds?circle-token={token}&limit={limit}",
        token = token,
        limit = 100,
    );

    let resp: reqwest::Response = reqwest::get(&url).await?;

    let builds = resp.json::<Vec<TryBuild>>().await?;

    let builds = builds
        .into_iter()
        .filter_map(TryBuild::into_build)
        .collect::<Vec<_>>();

    let repo = Repository::init(".")?;

    let builds = find_builds(builds, &repo);
    print_builds(builds, repo);

    Ok(())
}

fn current_branch_name(repo: &Repository) -> Result<Option<String>> {
    let head = repo.head()?;

    for branch in repo.branches(Some(BranchType::Local))? {
        if let Ok((branch, _)) = branch {
            if branch.get() == &head {
                if let Some(name) = branch.name()? {
                    return Ok(Some(name.to_string()));
                }
            }
        }
    }

    Ok(None)
}

fn find_builds(mut builds: Vec<Build>, repo: &Repository) -> Vec<Build> {
    builds.sort_unstable_by_key(|build| -build.build_num);

    let mut builds = builds
        .into_iter()
        .fold(Vec::<Build>::new(), |mut acc, build| {
            if acc.iter().find(|b| b.branch == build.branch).is_none() {
                acc.push(build);
            }
            acc
        })
        .into_iter()
        .filter(|build| repo.find_branch(&build.branch, BranchType::Local).is_ok())
        .collect::<Vec<_>>();

    builds.sort_unstable_by_key(|build| build.build_num);

    builds
}

fn print_builds(builds: Vec<Build>, repo: Repository) {
    let length = builds
        .iter()
        .max_by_key(|build| build.branch.len())
        .expect("failed to find longest branch")
        .branch
        .len();

    let current_branch_name = current_branch_name(&repo)
        .expect("failed to find current branch")
        .expect("failed to find current branch");

    for build in builds {
        let branch = if current_branch_name == build.branch {
            pad(&build.branch, length - build.branch.len())
                .magenta()
                .to_string()
        } else {
            pad(&build.branch, length - build.branch.len())
        };

        let build_num = build
            .outcome
            .as_ref()
            .map(|outcome| {
                if outcome.failed() {
                    format!("{}", build.build_num)
                } else {
                    String::new()
                }
            })
            .unwrap_or_else(String::new);

        println!(
            "{branch} {outcome} {build_num}",
            branch = branch,
            outcome = build
                .outcome
                .as_ref()
                .map(Outcome::term_string)
                .unwrap_or_else(|| "no outcome (yet)".to_string()),
            build_num = build_num,
        );
    }
}

fn pad(s: &str, n: usize) -> String {
    let mut acc = s.to_string();
    for _ in 0..n {
        acc = format!(" {}", acc);
    }
    acc
}

#[derive(Debug, Deserialize)]
struct TryBuild {
    branch: Option<String>,
    build_num: i32,
    outcome: Option<Outcome>,
}

impl TryBuild {
    fn into_build(self) -> Option<Build> {
        let branch = self.branch?;
        let build_num = self.build_num;
        let outcome = self.outcome;
        Some(Build {
            branch,
            build_num,
            outcome,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Build {
    branch: String,
    build_num: i32,
    outcome: Option<Outcome>,
}

#[derive(Debug, Deserialize)]
enum Outcome {
    #[serde(rename = "retried")]
    Retried,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "infrastructure_fail")]
    InfrastructureFail,
    #[serde(rename = "timedout")]
    Timedout,
    #[serde(rename = "not_run")]
    NotRun,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "scheduled")]
    Scheduled,
    #[serde(rename = "not_running")]
    NotRunning,
    #[serde(rename = "no_tests")]
    NoTests,
    #[serde(rename = "fixed")]
    Fixed,
    #[serde(rename = "success")]
    Success,
}

impl Outcome {
    fn term_string(&self) -> String {
        use Outcome::*;

        match self {
            Retried => "retried".to_string(),
            Canceled => "canceled".to_string(),
            InfrastructureFail => "infrastructure fail".red().to_string(),
            Timedout => "timeout".red().to_string(),
            NotRun => "not run".to_string(),
            Running => "running".blue().to_string(),
            Failed => "failed".red().to_string(),
            Queued => "queued".to_string(),
            Scheduled => "scheduled".magenta().to_string(),
            NotRunning => "not running".to_string(),
            NoTests => "no tests".to_string(),
            Fixed => "ok".green().to_string(),
            Success => "ok".green().to_string(),
        }
    }

    fn failed(&self) -> bool {
        match self {
            Outcome::Failed => true,
            _ => false,
        }
    }
}
