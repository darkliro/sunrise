//! Minecraft server DNS redirects (`ServerRedirectHandler` / `ServerNameResolver`).
//! Port 25565 asks for `_minecraft._tcp.<host>`'s SRV record; no record or a
//! failed query leaves the original address in use.
use std::net::IpAddr;

mod dns;

/// Where a server address leads (`ServerRedirectHandler`): its `_minecraft._tcp` SRV target when
/// the port is 25565 and the host a name, else the host and port as given. No address lookup.
pub fn server_redirect(host: &str, port: u16) -> (String, u16) {
    redirect_with(host, port, srv_redirect)
}

fn redirect_with(
    host: &str,
    port: u16,
    srv: impl FnOnce(&str) -> Option<(String, u16)>,
) -> (String, u16) {
    // Vanilla also asks for `_minecraft._tcp.127.0.0.1`, which never has a
    // record: IP literals and localhost skip the query.
    if port == 25565
        && host.parse::<IpAddr>().is_err()
        && host != "localhost"
        && let Some(target) = srv(&format!("_minecraft._tcp.{}.", host.trim_end_matches('.')))
    {
        // No second SRV lookup at the target.
        return target;
    }
    (host.to_owned(), port)
}

fn srv_redirect(name: &str) -> Option<(String, u16)> {
    dns::query_srv(name, &dns::system_name_servers(), dns::SERVER_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_port_follows_the_srv_record() {
        for (port, redirected, expected) in [
            (25565, false, ("play.example.org", 25565)),
            (25565, true, ("backend.example.org.", 25570)),
            (25566, false, ("play.example.org", 25566)),
        ] {
            let (host, port) = redirect_with("play.example.org", port, |name| {
                assert_eq!(name, "_minecraft._tcp.play.example.org.");
                redirected.then(|| ("backend.example.org.".into(), 25570))
            });
            assert_eq!((host.as_str(), port), expected);
        }
    }

    #[test]
    fn explicit_nondefault_ports_and_ip_literals_do_not_query_srv() {
        for (host, port) in [
            ("play.example.org", 25570),
            ("127.0.0.1", 25565),
            ("::1", 25565),
            ("localhost", 25565),
        ] {
            let redirected = redirect_with(host, port, |_| panic!("unexpected SRV query"));
            assert_eq!(redirected, (host.to_owned(), port));
        }
    }

    #[test]
    fn fully_qualified_host_does_not_get_a_second_trailing_dot() {
        redirect_with("play.example.org.", 25565, |name| {
            assert_eq!(name, "_minecraft._tcp.play.example.org.");
            None
        });
    }
}
