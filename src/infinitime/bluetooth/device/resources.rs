use super::{fs, InfiniTime, ProgressTx, ProgressTxWrapper};
// use std::sync::mpsc;
use std::io::{Cursor, Read};
// use futures::{pin_mut, StreamExt};
use anyhow::{anyhow, Result};
use serde::Deserialize;


#[derive(Deserialize, Debug)]
struct Resources {
    resources: Vec<Resource>,
}


#[derive(Deserialize, Debug)]
struct Resource {
    filename: String,
    path: String,
}


impl InfiniTime {
    pub async fn upload_resources(&self, resources_archive: &[u8], progress_sender: Option<ProgressTx>) -> Result<()>
    {
        let progress = ProgressTxWrapper(progress_sender);

        // Parse manifest from the archive
        let mut zip = zip::ZipArchive::new(Cursor::new(resources_archive))?;
        let mut json = String::new();
        zip.by_name("resources.json")?.read_to_string(&mut json)?;
        let manifest: Resources = serde_json::from_str(&json)
            .map_err(|_| anyhow!("Invalid resources.json"))?;

        // Make dirs
        let files = manifest.resources.iter().map(|r| r.path.as_str());
        for dir in fs::ancestors_union(files) {
            progress.report_msg(format!("Creating directory: {}", dir)).await;
            self.make_dir(dir).await?;
        }

        // Write files
        for res in manifest.resources {
            let mut content = Vec::new();
            progress.report_msg(format!("Writing resource file: {}", &res.path)).await;
            zip.by_name(&res.filename)?.read_to_end(&mut content)?;
            self.write_file(&res.path, &content, 0, progress.0.clone()).await?;
        }

        Ok(())
    }
}
