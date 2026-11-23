use std::{fs::File, io::Read, path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use url::Url;

use crate::runtime::MAX_MODULE_BYTES;

#[derive(Debug, PartialEq)]
pub enum Address {
    File(PathBuf),
    Web(Url),
}

impl Address {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            bail!("Enter a file path or an HTTP(S) URL.");
        }
        if value.contains("://") {
            let url = Url::parse(value).context("Invalid URL")?;
            match url.scheme() {
                "http" | "https" => Ok(Self::Web(url)),
                "file" => Ok(Self::File(
                    url.to_file_path()
                        .map_err(|_| anyhow::anyhow!("Invalid local file URL"))?,
                )),
                _ => bail!("Use a local path, file:// URL, or HTTP(S) URL."),
            }
        } else if let Some(rest) = value.strip_prefix("~/") {
            let home = std::env::var_os("HOME").context("Cannot expand ~ without HOME")?;
            Ok(Self::File(PathBuf::from(home).join(rest)))
        } else {
            Ok(Self::File(PathBuf::from(value)))
        }
    }
}

fn read_limited(reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_MODULE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MODULE_BYTES {
        bail!("Page exceeds the 4 MiB file limit.");
    }
    Ok(bytes)
}

pub fn load(value: &str) -> Result<Vec<u8>> {
    let source = match Address::parse(value)? {
        Address::File(path) => read_limited(
            File::open(&path).with_context(|| format!("Could not open {}", path.display()))?,
        )?,
        Address::Web(url) => {
            let response = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(20))
                .redirects(5)
                .build()
                .get(url.as_str())
                .set("Cache-Control", "no-cache")
                .set("Accept", "application/wasm, text/plain;q=0.9, */*;q=0.1")
                .call()
                .context("Could not fetch page")?;
            read_limited(response.into_reader()).context("Could not download page")?
        }
    };
    let wasm = wat::parse_bytes(&source).context("Page is not valid Wasm or WAT")?;
    if wasm.len() > MAX_MODULE_BYTES {
        bail!("Compiled page exceeds 4 MiB.");
    }
    Ok(wasm.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread};

    const PAGE: &str = "(module (func (export \"render\")))";

    #[test]
    fn parses_paths_urls_and_rejects_unsupported_addresses() {
        assert_eq!(
            Address::parse("some page.wasm").unwrap(),
            Address::File("some page.wasm".into())
        );
        assert_eq!(
            Address::parse("file:///tmp/some%20page.wasm").unwrap(),
            Address::File("/tmp/some page.wasm".into())
        );
        assert!(matches!(
            Address::parse("https://example.org/page.wasm").unwrap(),
            Address::Web(_)
        ));
        assert!(Address::parse("").is_err());
        assert!(Address::parse("ftp://example.org/page.wasm").is_err());
    }

    #[test]
    fn limits_downloads_without_trusting_content_length() {
        assert!(read_limited(&vec![0; MAX_MODULE_BYTES][..]).is_ok());
        assert!(read_limited(&vec![0; MAX_MODULE_BYTES + 1][..]).is_err());
    }

    #[test]
    fn loads_local_source_and_reloads_changed_bytes() {
        let path =
            std::env::temp_dir().join(format!("smolbrowse-loader-{}.wat", std::process::id()));
        std::fs::write(&path, PAGE).unwrap();
        let first = load(path.to_str().unwrap()).unwrap();
        std::fs::write(&path, "(module (func (export \"render\") nop))").unwrap();
        let second = load(Url::from_file_path(&path).unwrap().as_str()).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn fetches_http_follows_redirects_and_reports_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for response in [
                "HTTP/1.1 302 Found\r\nLocation: /cube.wasm\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}", PAGE.len()),
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 4096);
                }
                socket.write_all(response.as_bytes()).unwrap();
            }
        });
        let bytes = load(&format!("http://{address}/redirect")).unwrap();
        assert_eq!(bytes, wat::parse_str(PAGE).unwrap());
        assert!(load(&format!("http://{address}/missing"))
            .unwrap_err()
            .to_string()
            .contains("fetch"));
        server.join().unwrap();
    }
}
