use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use once_cell::sync::Lazy;
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("SM-Craft-Launcher/1.0")
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client")
});

pub fn client() -> &'static reqwest::Client {
    &CLIENT
}

#[derive(Clone, Debug)]
pub struct DlTask {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
}

impl DlTask {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        DlTask {
            url: url.into(),
            dest: dest.into(),
            sha1: None,
            sha256: None,
            sha512: None,
            size: None,
        }
    }
    pub fn with_sha1(mut self, s: Option<String>) -> Self {
        self.sha1 = s;
        self
    }
    pub fn with_sha512(mut self, s: Option<String>) -> Self {
        self.sha512 = s;
        self
    }
    pub fn with_size(mut self, s: Option<u64>) -> Self {
        self.size = s;
        self
    }
}

pub fn file_sha1(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

pub fn file_sha512(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha512::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// A cached file is accepted only when every supplied integrity check matches.
async fn already_good(task: &DlTask) -> bool {
    let meta = match tokio::fs::metadata(&task.dest).await {
        Ok(m) if m.is_file() => m,
        _ => return false,
    };
    if task.size.is_some_and(|sz| meta.len() != sz) { return false; }
    if let Some(want) = &task.sha1 {
        let p = task.dest.clone();
        let got = tokio::task::spawn_blocking(move || file_sha1(&p)).await;
        if !matches!(got, Ok(Ok(h)) if h.eq_ignore_ascii_case(want)) { return false; }
    }
    if let Some(want) = &task.sha256 {
        let p = task.dest.clone();
        let got = tokio::task::spawn_blocking(move || file_sha256(&p)).await;
        if !matches!(got, Ok(Ok(h)) if h.eq_ignore_ascii_case(want)) { return false; }
    }
    if let Some(want) = &task.sha512 {
        let p = task.dest.clone();
        let got = tokio::task::spawn_blocking(move || file_sha512(&p)).await;
        if !matches!(got, Ok(Ok(h)) if h.eq_ignore_ascii_case(want)) { return false; }
    }
    meta.len() > 0
}

pub type ByteCb = Arc<dyn Fn(u64, u64) + Send + Sync>;

pub async fn download_file_with(task: &DlTask, cb: Option<ByteCb>) -> Result<()> {
    if already_good(task).await {
        return Ok(());
    }
    if let Some(parent) = task.dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut last_err = anyhow!("download failed");
    for attempt in 0..3u32 {
        match try_download(task, cb.clone()).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt as u64 + 1)))
                    .await;
            }
        }
    }
    Err(last_err).with_context(|| format!("downloading {}", task.url))
}

async fn try_download(task: &DlTask, cb: Option<ByteCb>) -> Result<()> {
    let resp = client().get(&task.url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(task.size.unwrap_or(0));
    const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
    if total > MAX_DOWNLOAD_BYTES || task.size.is_some_and(|sz| sz > MAX_DOWNLOAD_BYTES) {
        return Err(anyhow!("download exceeds the 8 GiB safety limit"));
    }
    let part = task.dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let next = done.saturating_add(chunk.len() as u64);
        if next > task.size.unwrap_or(MAX_DOWNLOAD_BYTES) {
            drop(file);
            let _ = tokio::fs::remove_file(&part).await;
            return Err(anyhow!("download exceeded its allowed size"));
        }
        file.write_all(&chunk).await?;
        done = next;
        if let Some(cb) = &cb {
            cb(done, total);
        }
    }
    file.flush().await?;
    drop(file);

    if let Some(sz) = task.size {
        if done != sz {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(anyhow!("size mismatch: expected {sz}, got {done}"));
        }
    }
    if let Some(want) = &task.sha1 {
        let p = part.clone();
        let got = tokio::task::spawn_blocking(move || file_sha1(&p)).await??;
        if !got.eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(anyhow!("SHA-1 mismatch: expected {want}, got {got}"));
        }
    }
    if let Some(want) = &task.sha256 {
        let p = part.clone();
        let got = tokio::task::spawn_blocking(move || file_sha256(&p)).await??;
        if !got.eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(anyhow!("SHA-256 mismatch: expected {want}, got {got}"));
        }
    }
    if let Some(want) = &task.sha512 {
        let p = part.clone();
        let got = tokio::task::spawn_blocking(move || file_sha512(&p)).await??;
        if !got.eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(anyhow!("SHA-512 mismatch: expected {want}, got {got}"));
        }
    }
    tokio::fs::rename(&part, &task.dest).await?;
    Ok(())
}

pub async fn download_file(task: &DlTask) -> Result<()> {
    download_file_with(task, None).await
}

/// Downloads many files in parallel. `on_progress(done, total)` is called as
/// each file finishes. Returns an error listing failures after all tasks ran.
pub async fn download_many<F>(tasks: Vec<DlTask>, concurrency: usize, on_progress: F) -> Result<()>
where
    F: Fn(u64, u64) + Send + Sync,
{
    let total = tasks.len() as u64;
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicU64::new(0));
    let results: Vec<Result<()>> = futures::stream::iter(tasks)
        .map(|t| {
            let done = done.clone();
            async move {
                let r = download_file(&t).await;
                done.fetch_add(1, Ordering::SeqCst);
                r
            }
        })
        .buffer_unordered(concurrency.max(1))
        .inspect(|_| {
            on_progress(done.load(Ordering::SeqCst), total);
        })
        .collect()
        .await;
    let errors: Vec<String> = results
        .into_iter()
        .filter_map(|r| r.err())
        .map(|e| format!("{e:#}"))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "{} download(s) failed. First error: {}",
            errors.len(),
            errors[0]
        ))
    }
}

pub async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let r = client().get(url).send().await?.error_for_status()?;
    Ok(r.json::<T>().await?)
}
