use deepify::domain::{normalize_website, website_allowed, WhitelistEntry};
use deepify::services::{FakeClock, MockRestriction, SessionService};
use std::time::Duration;

#[test]
fn website_matching_covers_all_local_address_classes_and_public_ips() {
    let no_rules = Vec::new();
    for url in [
        "http://localhost:3000",
        "http://127.42.0.1",
        "http://10.255.1.2",
        "http://172.16.0.1",
        "http://172.31.255.254",
        "http://192.168.2.3",
        "http://169.254.1.2",
        "http://[::1]:8080",
        "http://[fc00::1234]",
        "http://[fd12::1]",
        "http://[fe80::1]",
    ] {
        assert!(
            website_allowed(url, &no_rules),
            "local address should be allowed: {url}"
        );
    }
    for url in [
        "https://8.8.8.8",
        "https://172.15.0.1",
        "https://172.32.0.1",
        "https://[2001:4860:4860::8888]",
    ] {
        assert!(
            !website_allowed(url, &no_rules),
            "public address needs an explicit rule: {url}"
        );
    }
}

#[test]
fn website_rules_ignore_scheme_and_port_and_validate_hosts_strictly() {
    let (host, path) = normalize_website("https://Example.COM:8443/docs").unwrap();
    assert_eq!((host.as_str(), path.as_str()), ("example.com", "/docs"));
    let rules = vec![WhitelistEntry::Website { host, path }];
    assert!(website_allowed(
        "http://sub.example.com:9999/docs/page",
        &rules
    ));
    assert!(!website_allowed("https://notexample.com/docs", &rules));
    for invalid in [
        "-bad.example",
        "bad-.example",
        ".example.com",
        "example..com",
        "999.999.1.1",
    ] {
        assert!(
            normalize_website(invalid).is_err(),
            "malformed host was accepted: {invalid}"
        );
    }
}

#[test]
fn paused_time_is_persisted_separately_without_increasing_focus() {
    let clock = FakeClock::new(100);
    let mut service = SessionService::new(clock.clone(), MockRestriction::default());
    service.start(300, None).unwrap();
    clock.advance(30);
    service.pause().unwrap();
    clock.advance(45);
    let resumed = service.resume().unwrap();
    assert_eq!(resumed.focused, Duration::from_secs(30));
    assert_eq!(resumed.paused, Duration::from_secs(45));
    assert!(resumed.restrictions_active);
}
