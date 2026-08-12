use std::path::Path;

use tokio::fs;

pub(crate) async fn write_sensitive_file(
    path: &Path,
    contents: &[u8],
) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use tokio::io::AsyncWriteExt;

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .await?;
        file.write_all(contents).await?;
        file.flush().await?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents).await?;
    }
    Ok(())
}
