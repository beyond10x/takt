//! The `api` transport of the GitLab forge: direct HTTPS calls to the host's REST API under the
//! credential the run file names. The token travels in one request header and nowhere else:
//! never in a URL or an argument line, never written, and replaced in every text that quotes the
//! host's answer or a transport error. A redirect is never followed, so the token goes to the
//! host named and no other. Plain HTTP is used only to a loopback host.

use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::super::Exit;
use super::super::credential::Secret;

/// The most bytes of one answer that are read.
const ANSWER_LIMIT: usize = 16 << 20;

/// How long a call may take, whole.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a connection may take to be made.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// The environment variable the credential helper of a push reads the token from; set for that
/// one line only.
pub const HELPER_VARIABLE: &str = "TAKT_FORGE_SECRET";

/// The credential helper of a push under the `api` transport: git asks it with `get` and it
/// answers from [`HELPER_VARIABLE`].
pub const CREDENTIAL_HELPER: &str = "credential.helper=!f() { test \"$1\" = get && printf 'username=oauth2\\npassword=%s\\n' \"$TAKT_FORGE_SECRET\"; }; f";

/// Whether `host`, with or without a port, is the machine itself.
fn is_loopback(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

/// The API of one GitLab host, called under one credential.
pub struct Api {
    base: String,
    loopback: bool,
    secret: Secret,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Api")
            .field("base", &self.base)
            .field("secret", &self.secret)
            .finish()
    }
}

impl Api {
    /// The API of `host` (with its port, when it has one) under `secret`.
    #[must_use]
    pub fn new(host: &str, secret: Secret) -> Self {
        let loopback = is_loopback(host);
        let scheme = if loopback { "http" } else { "https" };
        Self {
            base: format!("{scheme}://{host}/api/v4"),
            loopback,
            secret,
        }
    }

    /// The token, for the one credential-helper line of a push.
    #[must_use]
    pub fn secret(&self) -> &Secret {
        &self.secret
    }

    /// The URL of `path` of the API.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }

    /// Calls `method` on `path` with `body`, a JSON document, when there is one. The answer is
    /// an [`Exit`] as `glab api` makes one: exit 0 and the body on a 2xx status, else exit 1,
    /// the body and standard error that ends `(HTTP <status>)`. The token is replaced in both.
    pub fn send(&self, method: &str, path: &str, body: Option<Vec<u8>>) -> Result<Exit> {
        let url = self.url(path);
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| anyhow!("{method} is no HTTP method"))?;
        let described = format!("{method} {url}");
        let outcome = std::thread::scope(|scope| {
            scope
                .spawn(|| self.exchange(&method, &url, body))
                .join()
                .map_err(|_| anyhow!("the call to the API panicked"))
        })?;
        match outcome {
            Ok((status, answer)) => {
                let text = self.secret.redact(&String::from_utf8_lossy(&answer));
                if (200..300).contains(&status) {
                    Ok(Exit {
                        code: Some(0),
                        stdout: text,
                        stderr: String::new(),
                    })
                } else {
                    Ok(Exit {
                        code: Some(1),
                        stdout: text,
                        stderr: format!("{described} (HTTP {status})"),
                    })
                }
            }
            Err(error) => Err(anyhow!(
                "{}",
                self.secret
                    .redact(&format!("{described} did not answer: {error:#}"))
            )),
        }
    }

    /// One call on a runtime of its own, in the thread that runs it: a caller inside a runtime
    /// is no obstacle to it.
    fn exchange(
        &self,
        method: &reqwest::Method,
        url: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(u16, Vec<u8>)> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let mut client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(CALL_TIMEOUT)
                .user_agent(concat!("takt/", env!("CARGO_PKG_VERSION")));
            if self.loopback {
                client = client.no_proxy();
            }
            let mut token = reqwest::header::HeaderValue::from_str(self.secret.expose())
                .map_err(|_| anyhow!("the credential is no header value"))?;
            token.set_sensitive(true);
            let mut request = client
                .build()?
                .request(method.clone(), url)
                .header("PRIVATE-TOKEN", token)
                .header(reqwest::header::ACCEPT, "application/json");
            if let Some(body) = body {
                request = request
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body);
            }
            let mut response = request.send().await?;
            let status = response.status().as_u16();
            let mut answer = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if answer.len() + chunk.len() > ANSWER_LIMIT {
                    return Err(anyhow!("the answer is longer than {ANSWER_LIMIT} bytes"));
                }
                answer.extend_from_slice(&chunk);
            }
            Ok((status, answer))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_for_loopback_only() {
        for host in ["127.0.0.1:8080", "localhost", "[::1]:9", "LOCALHOST:1"] {
            assert!(
                Api::new(host, Secret::new("x")).base.starts_with("http://"),
                "{host}"
            );
        }
        for host in ["gitlab.example.com", "gitlab.example.com:8443", "10.0.0.1"] {
            assert!(
                Api::new(host, Secret::new("x"))
                    .base
                    .starts_with("https://"),
                "{host}"
            );
        }
    }
}
