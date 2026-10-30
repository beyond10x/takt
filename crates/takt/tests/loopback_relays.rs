//! story:git-and-model-through-loopback-relays, at the function level: git reaches a remote over
//! plain `http://` only when its host is a loopback host, a relay beside the run that holds the
//! forge's credential, and a `--harness harness` endpoint on a loopback host runs with no
//! credential, the model's key held by the relay. Every other `http://` URL is refused naming it,
//! and every other endpoint keeps its credential requirement. Nothing here opens a connection.

use takt::git::{RemoteTransport, remote_transport};
use takt::harness::{Credential, EndpointConfig, EndpointFields, RouteEndpoint};
use takt::run_config::Routes;

/// `http://` to a loopback host, by name, by an address in `127.0.0.0/8` and by `[::1]`, with
/// or without a port or a userinfo, is a loopback relay.
#[test]
fn http_to_a_loopback_host_is_a_relay() {
    for url in [
        "http://127.0.0.1:8403/o/r.git",
        "http://127.0.0.2/o/r.git",
        "http://localhost:8403/o/r.git",
        "http://LOCALHOST/o/r",
        "http://[::1]:8403/o/r.git",
        "http://user@127.0.0.1:8403/o/r.git",
        "http://127.0.0.1:8403",
    ] {
        assert_eq!(
            remote_transport(url).expect("admitted"),
            RemoteTransport::LoopbackHttp,
            "{url}"
        );
    }
    assert_eq!(
        RemoteTransport::LoopbackHttp.allowed_protocols(),
        "http:https"
    );
}

/// `https://` stays HTTPS alone, and SSH, a path and `file://` are no HTTP at all.
#[test]
fn https_and_other_transports_are_told_apart() {
    assert_eq!(
        remote_transport("https://example.com/o/r.git").expect("admitted"),
        RemoteTransport::Https
    );
    assert_eq!(RemoteTransport::Https.allowed_protocols(), "https");
    for url in [
        "ssh://git@example.com/o/r.git",
        "git@example.com:o/r.git",
        "/srv/git/r.git",
        "file:///srv/git/r.git",
    ] {
        assert_eq!(
            remote_transport(url).expect("admitted"),
            RemoteTransport::Other,
            "{url}"
        );
    }
}

/// Acceptance 1's refusal: `http://` to any host that is not a loopback host is refused, naming
/// the URL without its userinfo, which may hold a password.
#[test]
fn http_to_any_other_host_is_refused_naming_the_url() {
    for (url, shown) in [
        ("http://example.com/o/r.git", "http://example.com/o/r.git"),
        ("HTTP://example.com/o/r.git", "HTTP://example.com/o/r.git"),
        (
            "http://u:pass-word@example.com/o/r.git",
            "http://example.com/o/r.git",
        ),
        (
            "http://127.0.0.1.example.com/o/r.git",
            "http://127.0.0.1.example.com/o/r.git",
        ),
        (
            "http://10.0.0.1:8403/o/r.git",
            "http://10.0.0.1:8403/o/r.git",
        ),
        (
            "http://localhost.:8403/o/r.git",
            "http://localhost.:8403/o/r.git",
        ),
    ] {
        let error = remote_transport(url).expect_err(url);
        let message = format!("{error:#}");
        assert!(message.contains(shown), "{url}: {message}");
        assert!(message.contains("no loopback host"), "{url}: {message}");
        assert!(!message.contains("pass-word"), "{url}: {message}");
    }
}

fn base_url(url: &str) -> EndpointFields {
    EndpointFields {
        base_url: Some(url.to_owned()),
        ..EndpointFields::default()
    }
}

/// Acceptance 2, takt's side: a base URL on a loopback host with no credential flag or file
/// points the session at no credential, no token file and no secret's name, and the record says
/// `none`.
#[test]
fn a_loopback_endpoint_with_no_credential_names_none() {
    let home = std::path::Path::new("/srv/user-home");
    for url in [
        "http://127.0.0.1:8401/v1",
        "http://localhost:8401/v1",
        "https://127.0.0.1:8443/v1",
    ] {
        let config = EndpointConfig::of(&base_url(url), None, Some(home)).expect(url);
        assert_eq!(config.credential, Credential::None, "{url}");
        let endpoint = config.session_endpoint();
        assert!(endpoint.token_file.as_os_str().is_empty(), "{url}");
        assert_eq!(endpoint.token_pointer, None, "{url}");
        assert_eq!(config.api_key_secret(), None, "{url}");
        config.check_token().expect("no token to check");
        let recorded = serde_json::to_value(&config).expect("the endpoint records");
        assert_eq!(
            recorded["credential"],
            serde_json::json!({"kind": "none"}),
            "{url}"
        );
    }
}

/// A credential given for a loopback endpoint is still the one it names.
#[test]
fn a_loopback_endpoint_keeps_a_credential_it_is_given() {
    let mut fields = base_url("http://127.0.0.1:8401/v1");
    fields.secret = Some("llm/relay".to_owned());
    let config = EndpointConfig::of(&fields, None, None).expect("admitted");
    assert_eq!(config.api_key_secret().as_deref(), Some("llm/relay"));
}

/// Acceptance 3: `http://` to any other host is refused as before, and an `https://` endpoint on
/// another host with no credential is refused as before: the default token goes to the default
/// base URL alone.
#[test]
fn any_other_endpoint_keeps_its_credential_requirement() {
    let home = std::path::Path::new("/srv/user-home");
    let error = EndpointConfig::of(&base_url("http://example.com/v1"), None, Some(home))
        .expect_err("plain http off the machine");
    let message = format!("{error:#}");
    assert!(message.contains("http://example.com/v1"), "{message}");
    assert!(message.contains("no loopback host"), "{message}");
    let error = EndpointConfig::of(&base_url("https://example.com/v1"), None, Some(home))
        .expect_err("no credential off the machine");
    let message = format!("{error:#}");
    assert!(message.contains("https://example.com/v1"), "{message}");
    assert!(message.contains("no credential is given"), "{message}");
}

/// An `llm.routes/1` document with one anonymous endpoint at `base`, on the `messages` protocol,
/// and one route `relay` to it.
fn anonymous(base: &str) -> Routes {
    let document = serde_json::json!({
        "format": "llm.routes/1",
        "endpoints": {
            "boundary": {
                "base_url": base,
                "protocol": "messages",
                "auth": {"kind": "anonymous"},
            },
        },
        "routes": {"relay": "boundary/a-model"},
    });
    Routes::parse(&document.to_string()).expect("llm parses")
}

/// An anonymous route to a loopback host resolves with no credential and no key; on the
/// `messages` wire, since no key is staged.
#[test]
fn an_anonymous_route_to_a_loopback_host_resolves_with_no_credential() {
    let route = RouteEndpoint::resolve(&anonymous("http://127.0.0.1:8401/v1"), "relay", None)
        .expect("admitted");
    assert_eq!(route.config.credential, Credential::None);
    assert_eq!(route.config.wire, "anthropic-messages");
    assert_eq!(route.config.model, "a-model");
    assert!(route.secret.is_none());
}

/// An anonymous route to any other host is refused, naming the route: takt sends no endpoint off
/// the machine without a credential.
#[test]
fn an_anonymous_route_to_another_host_is_refused() {
    let error = RouteEndpoint::resolve(&anonymous("https://example.com/v1"), "relay", None)
        .err()
        .expect("refused");
    let message = format!("{error:#}");
    assert!(message.contains("relay"), "{message}");
    assert!(message.contains("names no credential"), "{message}");
}
