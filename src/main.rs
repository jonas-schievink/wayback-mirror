//! TODO: Write crate docs

#![warn(rust_2018_idioms)]

use std::{
    collections::{hash_map::Entry, HashMap},
    fs::File,
    path::{Path, PathBuf},
};

use futures::StreamExt;
use structopt::StructOpt;
use surf::Url;

use crate::downloader::{DownloadJob, DownloadPlan};

mod api;
mod downloader;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Error>;

const DEFAULT_PAGES: usize = 100;
const DEFAULT_PAGE_CONCURRENCY: usize = 10;

#[derive(StructOpt)]
struct Args {
    /// Output directory.
    #[structopt(short, long)]
    out_dir: PathBuf,

    /// URL of the original website to mirror.
    url: String,
}

#[async_std::main]
async fn main() -> Result<()> {
    let args = Args::from_args();

    let plan_path = args.out_dir.join(".rayback.json");
    let download_plan = match recover_plan(&plan_path) {
        Ok(plan) => {
            log::info!("loaded existing download plan from {}", plan_path.display());
            plan
        }
        Err(e) => {
            log::info!("could not open preexisting download plan: {}", e);
            let plan = download_plan(&args).await?;

            std::fs::create_dir_all(&args.out_dir)?;
            serde_json::to_writer(File::create(&plan_path)?, &plan)?;
            log::info!("download plan saved as {}", plan_path.display());

            plan
        }
    };

    download_plan
        .execute(&args.out_dir, DEFAULT_PAGE_CONCURRENCY)
        .await?;

    Ok(())
}

fn recover_plan(path: &Path) -> Result<DownloadPlan> {
    Ok(serde_json::from_reader(File::open(path)?)?)
}

async fn download_plan(args: &Args) -> Result<DownloadPlan> {
    let url = &args.url;

    println!("Fetching archive records for {}", url);

    let mut resources: HashMap<_, (DownloadJob, String)> = HashMap::new();
    let progress = indicatif::ProgressBar::new(DEFAULT_PAGES as _);

    let stream = api::query_snapshots(url, DEFAULT_PAGES, DEFAULT_PAGE_CONCURRENCY);
    let url = Url::parse(url)?;
    let mut stream = Box::pin(stream);
    while let Some(page) = stream.next().await {
        let page = page?;
        for entry in page {
            let rel_path = sanitize_path(PathBuf::from(&entry.original.path()[url.path().len()..]));
            let job = DownloadJob {
                rel_path,
                url: format!(
                    "https://web.archive.org/web/{timestamp}id_/{original}",
                    timestamp = entry.timestamp,
                    original = entry.original,
                ),
            };
            match resources.entry(entry.original.clone()) {
                Entry::Occupied(mut occupied) => {
                    let prev_timestamp = &occupied.get().1;
                    if prev_timestamp < &entry.timestamp {
                        log::debug!(
                            "{} is older than {}, replacing {} with {}",
                            prev_timestamp,
                            entry.timestamp,
                            occupied.get().0.url,
                            job.url,
                        );
                        occupied.insert((job, entry.timestamp));
                    }
                }
                Entry::Vacant(vacant) => {
                    vacant.insert((job, entry.timestamp));
                }
            }
        }
        progress.inc(1);
    }
    drop(progress);

    println!();
    println!("Index fetched, {} total resources", resources.len());

    let mut jobs: Vec<_> = resources.values().map(|(job, _)| job.clone()).collect();
    jobs.sort_by(|a, b| a.url.cmp(&b.url));

    Ok(DownloadPlan::new(jobs.into_iter()))
}

/// The wayback API returns URLs that correspond to directories, not files. This function tries to
/// detect those and add `index.html` to them, so that they can be browsed locally (or hosted by an
/// HTTP server).
fn sanitize_path(mut path: PathBuf) -> PathBuf {
    if path.is_dir() || path.as_os_str().is_empty() {
        path.push("index.html");
        return path;
    }
    if path.file_name() == path.file_stem() {
        // No file ending, assume directory.
        // FIXME: this mirrors what wayback-machine-downloader does, but checking if `path` with a
        // trailing `/` is also in the index might be a better indication to skip `path` here
        // (instead of duplicating `index.html`)
        path.push("index.html");
        return path;
    }
    path
}
