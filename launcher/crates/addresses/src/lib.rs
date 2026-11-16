//! The outside addresses the launcher has built in, vanilla's aside, in one place.

/// The players' page on connecting an AI agent (the website's docs/agents.md).
pub const AGENT_DOCS: &str = "https://playsunrise.net/docs/agents";

/// The mod registry's policy, which the launcher's mod pages link.
pub const MOD_POLICY: &str = "https://playsunrise.net/mods/policy";

/// The telemetry ingest server (`crates/sunrise-telemetry-server` in the game repository): opt-in
/// telemetry, the "I'm playing" ping, benchmark uploads (`SUNRISE_TELEMETRY_URL` overrides it).
pub const TELEMETRY: &str = "https://telemetry.playsunrise.net";

/// The GitHub profile of who made the launcher's look, credited in Settings' About (its picture is
/// `<this>.png`).
pub const LAUNCHER_LOOK_CREDIT: &str = "https://github.com/Xmilon";

/// The official launcher installer's product home.
pub const LAUNCHER_ABOUT: &str = "https://playsunrise.net";

/// The separately branded arismoko fork launcher installer's product home.
pub const FORK_LAUNCHER_ABOUT: &str = "https://github.com/sunrise-org/sunrise";

/// Where the game repository publishes each game build's mod SDK (`sdk-<build id>`) and the game
/// releases whose `sdk.json` names their build; a release's tag and file follow.
/// `SUNRISE_SDK_BASE` overrides it.
pub const SDK_RELEASES: &str = "https://github.com/sunrise-org/sunrise/releases/download";
