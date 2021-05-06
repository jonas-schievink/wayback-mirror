//! Resumable, asynchronous batch downloads.

use async_std::stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadJob {
    pub rel_path: PathBuf,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct DownloadPlan {
    list: Vec<DownloadJob>,
}

impl DownloadPlan {
    pub fn new(jobs: impl Iterator<Item = DownloadJob>) -> Self {
        Self {
            list: jobs.collect(),
        }
    }

    pub async fn execute(&self, out_dir: &Path, concurrency: usize) -> Result<()> {
        let progress = indicatif::ProgressBar::new(self.list.len() as _);
        let stream = stream::from_iter(self.list.iter().map(|entry| async move {
            let path = out_dir.join(&entry.rel_path);

            // If we've resumed a download and the file already exists, don't download it again.
            if async_std::path::Path::new(&path).exists().await {
                return Ok(());
            }

            let mut resp = surf::get(&entry.url).send().await?;

            if let Some(dir) = path.parent() {
                async_std::fs::create_dir_all(dir).await?;
            }

            // Download to a temporary file, then rename
            let mut tmp_path = path.clone();
            tmp_path.set_extension("raybackdl");
            let mut file = async_std::fs::File::create(&tmp_path).await?;

            async_std::io::copy(&mut resp, &mut file).await?;
            async_std::fs::rename(&tmp_path, &path).await?;

            Ok::<_, crate::Error>(())
        }))
        .buffer_unordered(concurrency);

        let mut stream = Box::pin(stream);
        while let Some(resource) = stream.next().await {
            resource?;
            progress.inc(1);
        }
        drop(progress);

        println!("Page downloaded successfully");
        Ok(())
    }
}
