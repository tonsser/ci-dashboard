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

fn main() {
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
        limit = 50,
    );

    let mut resp = reqwest::get(&url).expect("failed to call circleci api");

    let builds = match resp.json::<Vec<Build>>() {
        Ok(builds) => builds,
        Err(e) => {
            eprintln!("{:?}", e);
            panic!("failed to parse response as json")
        }
    };

    let builds = find_builds(builds);
    print_builds(builds);
}

fn find_builds(mut builds: Vec<Build>) -> Vec<Build> {
    builds.sort_unstable_by_key(|build| -build.build_num);

    let repo = Repository::init(".").expect("No .git folder found");

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

    // builds.sort_unstable_by_key(|build| {
    //     if &build.branch == "master" {
    //         "aaaa".to_string()
    //     } else if &build.branch == "staging" {
    //         "aaaaaaa".to_string()
    //     } else if &build.branch == "develop" {
    //         "aaaaaaaaaa".to_string()
    //     } else {
    //         build.branch.clone()
    //     }
    // });

    builds
}

fn print_builds(builds: Vec<Build>) {
    let length = builds
        .iter()
        .max_by_key(|build| build.branch.len())
        .expect("failed to find longest branch")
        .branch
        .len();

    for build in builds {
        let branch = pad(&build.branch, length - build.branch.len());
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
                .unwrap_or_else(|| "null".to_string()),
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
