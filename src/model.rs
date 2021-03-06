//! Models to (de)serialize incoming/outgoing websocket events and HTTP
//! responses.

use serde::{Deserialize, Serialize};

/// The type of event that something is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Opcode {
    /// A combined voice server and voice state update.
    VoiceUpdate,
    /// Retrieve a player.
    #[serde(rename = "get-player")]
    GetPlayer,
    /// Play a track.
    Play,
    /// Stop a player.
    Stop,
    /// Update a player.
    Update,
    /// Destroy a player.
    Destroy,
    /// An update about a player's current track.
    PlayerUpdate,
    /// Meta information about a track starting or ending.
    Event,
    /// Updated statistics about a node.
    Stats,
}

pub mod outgoing {
    //! Events that clients send to Lavalink.

    use super::Opcode;
    use serde::{Deserialize, Serialize};
    use serde_with::skip_serializing_none;
    use twilight_model::{gateway::payload::VoiceServerUpdate, id::GuildId};

    /// An outgoing event to send to Lavalink.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(untagged)]
    pub enum OutgoingEvent {
        /// A combined voice server and voice state update.
        VoiceUpdate(VoiceUpdate),
        /// Retrieve a player.
        GetPlayer(GetPlayer),
        /// Play a track.
        Play(Play),
        /// Stop a player.
        Stop(Stop),
        /// Update a player.
        Update(Update),
        /// Destroy a player for a guild.
        Destroy(Destroy),
    }

    impl OutgoingEvent {
        /// Get the event opcode.
        pub fn op(&self) -> Opcode {
            match self {
                OutgoingEvent::VoiceUpdate(data) => data.op,
                OutgoingEvent::GetPlayer(data) => data.op,
                OutgoingEvent::Play(data) => data.op,
                OutgoingEvent::Stop(data) => data.op,
                OutgoingEvent::Update(data) => data.op,
                OutgoingEvent::Destroy(data) => data.op,
            }
        }

        /// Get the event guild id.
        pub fn guild_id(&self) -> GuildId {
            match self {
                OutgoingEvent::VoiceUpdate(data) => data.guild_id,
                OutgoingEvent::GetPlayer(data) => data.guild_id,
                OutgoingEvent::Play(data) => data.guild_id,
                OutgoingEvent::Stop(data) => data.guild_id,
                OutgoingEvent::Update(data) => data.guild_id,
                OutgoingEvent::Destroy(data) => data.guild_id,
            }
        }
    }

    impl From<VoiceUpdate> for OutgoingEvent {
        fn from(event: VoiceUpdate) -> OutgoingEvent {
            Self::VoiceUpdate(event)
        }
    }

    impl From<GetPlayer> for OutgoingEvent {
        fn from(event: GetPlayer) -> OutgoingEvent {
            Self::GetPlayer(event)
        }
    }

    impl From<Play> for OutgoingEvent {
        fn from(event: Play) -> OutgoingEvent {
            Self::Play(event)
        }
    }

    impl From<Stop> for OutgoingEvent {
        fn from(event: Stop) -> OutgoingEvent {
            Self::Stop(event)
        }
    }

    impl From<Update> for OutgoingEvent {
        fn from(event: Update) -> OutgoingEvent {
            Self::Update(event)
        }
    }

    impl From<Destroy> for OutgoingEvent {
        fn from(event: Destroy) -> OutgoingEvent {
            Self::Destroy(event)
        }
    }

    /// A combined voice server and voice state update.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct VoiceUpdate {
        /// The opcode of the event.
        pub op: Opcode,
        /// The session ID of the voice channel.
        pub session_id: String,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The inner event being forwarded to a node.
        pub event: SlimVoiceServerUpdate,
    }

    impl VoiceUpdate {
        /// Create a new voice update event.
        pub fn new(
            guild_id: GuildId,
            session_id: impl Into<String>,
            event: SlimVoiceServerUpdate,
        ) -> Self {
            Self {
                op: Opcode::VoiceUpdate,
                session_id: session_id.into(),
                guild_id,
                event,
            }
        }
    }

    /// A slimmed version of a twilight voice server update.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub struct SlimVoiceServerUpdate {
        /// The endpoint of the Discord voice server.
        pub endpoint: Option<String>,
        /// The authentication token used by the bot to connect to the Discord
        /// voice server.
        pub token: String,
    }

    impl From<VoiceServerUpdate> for SlimVoiceServerUpdate {
        fn from(update: VoiceServerUpdate) -> Self {
            Self {
                endpoint: update.endpoint,
                token: update.token,
            }
        }
    }

    /// Retrieve a player.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct GetPlayer {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
    }

    impl GetPlayer {
        /// Create a new voice update event.
        pub fn new(guild_id: GuildId) -> Self {
            Self {
                op: Opcode::GetPlayer,
                guild_id,
            }
        }
    }

    /// Play a track, optionally specifying to not skip the current track.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Play {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The base64 track information.
        pub track: String,
        /// The position in milliseconds to start the track from.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start_time: Option<u64>,
        /// The position in milliseconds to end the track. Does nothing.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_time: Option<u64>,
        /// Whether or not to replace the currently playing track with this new
        /// track.
        ///
        /// Set to `true` to keep playing the current playing track, or `false`
        /// to replace the current playing track with a new one.
        pub no_replace: bool,
    }

    impl Play {
        /// Create a play event.
        pub fn new(guild_id: GuildId, track: impl Into<String>) -> Self {
            Self::new_complex(guild_id, track, None, None, false)
        }

        /// Create a new complex play event.
        pub fn new_complex(
            guild_id: GuildId,
            track: impl Into<String>,
            start_time: impl Into<Option<u64>>,
            end_time: impl Into<Option<u64>>,
            no_replace: bool,
        ) -> Self {
            Self {
                op: Opcode::Play,
                guild_id,
                track: track.into(),
                start_time: start_time.into(),
                end_time: end_time.into(),
                no_replace,
            }
        }
    }

    /// Stop a player.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Stop {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
    }

    impl Stop {
        /// Create a new stop event.
        pub fn new(guild_id: GuildId) -> Self {
            Self {
                guild_id,
                op: Opcode::Stop,
            }
        }
    }

    /// Set the filters of a player
    #[skip_serializing_none]
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Filters {
        /// The karaoke filter.
        pub karaoke: Option<Karaoke>,
        /// The timescale filter.
        pub timescale: Option<Timescale>,
        /// The tremolo filter.
        pub tremolo: Option<Tremolo>,
        /// The vibrato filter.
        pub vibrato: Option<Vibrato>,
        /// The equalizer filter.
        pub equalizer: Option<Equalizer>,
        /// The volume filter, always None.
        #[serde(skip)]
        pub volume: Option<()>,
    }

    impl Filters {
        /// Create new filters.
        pub fn new(
            karaoke: impl Into<Option<Karaoke>>,
            timescale: impl Into<Option<Timescale>>,
            tremolo: impl Into<Option<Tremolo>>,
            vibrato: impl Into<Option<Vibrato>>,
            equalizer: impl Into<Option<Equalizer>>,
        ) -> Self {
            Self {
                karaoke: karaoke.into(),
                timescale: timescale.into(),
                tremolo: tremolo.into(),
                vibrato: vibrato.into(),
                equalizer: equalizer.into(),
                volume: None,
            }
        }
    }

    impl Default for Filters {
        fn default() -> Self {
            Self::new(
                Karaoke {
                    level: 0.0,
                    mono_level: 0.0,
                    filter_band: 0.0,
                    filter_width: 0.0,
                    enabled: false,
                },
                Timescale {
                    speed: 0.0,
                    pitch: 0.0,
                    rate: 0.0,
                    enabled: false,
                },
                Tremolo {
                    frequency: 0.0,
                    depth: 0.0,
                    enabled: false,
                },
                Vibrato {
                    frequency: 0.0,
                    depth: 0.0,
                    enabled: false,
                },
                Equalizer {
                    bands: vec![],
                    enabled: false,
                },
            )
        }
    }

    /// Karaoke filter.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Karaoke {
        /// The effect level.
        pub level: f64,
        /// The mono effect level.
        pub mono_level: f64,
        /// The filter band.
        pub filter_band: f64,
        /// The filter width.
        pub filter_width: f64,
        /// Whether is enabled, always false.
        #[serde(skip)]
        pub enabled: bool,
    }

    impl Karaoke {
        /// Create a new karaoke filter.
        pub fn new(level: f64, mono_level: f64, filter_band: f64, filter_width: f64) -> Self {
            Self {
                level,
                mono_level,
                filter_band,
                filter_width,
                enabled: false,
            }
        }
    }

    /// Timescale filter.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Timescale {
        /// Speed to play at.
        pub speed: f64,
        /// Pitch to play at.
        pub pitch: f64,
        /// Rate to play at.
        pub rate: f64,
        /// Whether is enabled, always false.
        #[serde(skip)]
        pub enabled: bool,
    }

    impl Timescale {
        /// Create a new timescale filter.
        pub fn new(speed: f64, pitch: f64, rate: f64) -> Self {
            Self {
                speed,
                pitch,
                rate,
                enabled: false,
            }
        }
    }

    /// Tremolo filter.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Tremolo {
        /// The filter frequency.
        pub frequency: f64,
        /// The filter depth.
        pub depth: f64,
        /// Whether is enabled, always false.
        #[serde(skip)]
        pub enabled: bool,
    }

    impl Tremolo {
        /// Create a new tremolo filter.
        pub fn new(frequency: f64, depth: f64) -> Self {
            Self {
                frequency,
                depth,
                enabled: false,
            }
        }
    }

    /// Vibrato filter.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Vibrato {
        /// The filter frequency.
        pub frequency: f64,
        /// The filter depth.
        pub depth: f64,
        /// Whether is enabled, always false.
        #[serde(skip)]
        pub enabled: bool,
    }

    impl Vibrato {
        /// Create a new timescale filter.
        pub fn new(frequency: f64, depth: f64) -> Self {
            Self {
                frequency,
                depth,
                enabled: false,
            }
        }
    }

    /// Equalize a player.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Equalizer {
        /// The bands to use as part of the equalizer.
        pub bands: Vec<EqualizerBand>,
        /// Whether is enabled, always false.
        #[serde(skip)]
        pub enabled: bool,
    }

    impl Equalizer {
        /// Create a new equalizer filter.
        pub fn new(bands: Vec<EqualizerBand>) -> Self {
            Self {
                bands,
                enabled: false,
            }
        }
    }

    /// A band of the equalizer.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EqualizerBand {
        /// The band.
        pub band: i64,
        /// The gain.
        pub gain: f64,
    }

    /// Update a player.
    #[skip_serializing_none]
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Update {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// Whether to pause the player.
        pub pause: Option<bool>,
        /// The new position of the player.
        pub position: Option<i64>,
        /// The volume of the player from 0 to 1000. 100 is the default.
        pub volume: Option<i64>,
        /// The filters of the player.
        pub filters: Option<Filters>,
    }

    impl Update {
        /// Create a new update event.
        pub fn new(
            guild_id: GuildId,
            pause: impl Into<Option<bool>>,
            position: impl Into<Option<i64>>,
            volume: impl Into<Option<i64>>,
            filters: impl Into<Option<Filters>>,
        ) -> Self {
            Self {
                op: Opcode::Update,
                guild_id,
                pause: pause.into(),
                position: position.into(),
                volume: volume.into(),
                filters: filters.into(),
            }
        }
    }

    /// Destroy a player from a node.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Destroy {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
    }

    impl Destroy {
        /// Create a new destroy event.
        pub fn new(guild_id: GuildId) -> Self {
            Self {
                op: Opcode::Destroy,
                guild_id,
            }
        }
    }
}

pub mod incoming {
    //! Events that Lavalink sends to clients.

    use super::outgoing::Filters;
    use super::Opcode;
    use crate::http::Error;
    use serde::{Deserialize, Serialize};
    use twilight_model::id::GuildId;

    /// An incoming event from a Lavalink node.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(untagged)]
    pub enum IncomingEvent {
        /// An update about the information of a player.
        PlayerUpdate(PlayerUpdate),
        /// New statistics about a node and its host.
        Stats(Stats),
        /// A track ended.
        TrackEnd(TrackEnd),
        /// A track started.
        TrackStart(TrackStart),
        /// A track encountered exception.
        TrackException(TrackException),
        /// A track got stuck.
        TrackStuck(TrackStuck),
        /// A websocket got closed.
        WebsocketClose(WebsocketClose),
        /// A player got destroyed.
        PlayerDestroy(PlayerDestroy),
    }

    impl IncomingEvent {
        /// Get the event opcode.
        pub fn op(&self) -> Opcode {
            match self {
                IncomingEvent::PlayerUpdate(data) => data.op,
                IncomingEvent::Stats(data) => data.op,
                IncomingEvent::TrackEnd(data) => data.op,
                IncomingEvent::TrackStart(data) => data.op,
                IncomingEvent::TrackException(data) => data.op,
                IncomingEvent::TrackStuck(data) => data.op,
                IncomingEvent::WebsocketClose(data) => data.op,
                IncomingEvent::PlayerDestroy(data) => data.op,
            }
        }

        /// Get the event guild id.
        pub fn guild_id(&self) -> GuildId {
            match self {
                IncomingEvent::PlayerUpdate(data) => data.guild_id,
                IncomingEvent::Stats(_) => GuildId::default(),
                IncomingEvent::TrackEnd(data) => data.guild_id,
                IncomingEvent::TrackStart(data) => data.guild_id,
                IncomingEvent::TrackException(data) => data.guild_id,
                IncomingEvent::TrackStuck(data) => data.guild_id,
                IncomingEvent::WebsocketClose(data) => data.guild_id,
                IncomingEvent::PlayerDestroy(data) => data.guild_id,
            }
        }
    }

    impl From<PlayerUpdate> for IncomingEvent {
        fn from(event: PlayerUpdate) -> IncomingEvent {
            Self::PlayerUpdate(event)
        }
    }

    impl From<Stats> for IncomingEvent {
        fn from(event: Stats) -> IncomingEvent {
            Self::Stats(event)
        }
    }

    /// An update about the information of a player.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PlayerUpdate {
        /// The opcode of the event.
        pub op: Opcode,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The new state of the player.
        pub state: PlayerUpdateState,
    }

    /// New statistics about a node and its host.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PlayerUpdateState {
        /// The new time of the player.
        pub time: i64,
        /// The new position of the player.
        pub position: Option<i64>,
        /// Whether the player is paused.
        pub paused: bool,
        /// Volume of the player.
        pub volume: i64,
        /// Filters present.
        pub filters: Filters,
        /// Whether the player is destroyed.
        pub destroyed: Option<bool>,
        /// Mixer, always None.
        #[serde(skip)]
        pub mixer: Option<()>,
        /// Mixer enabled, always None.
        #[serde(skip)]
        pub mixer_enabled: Option<()>,
        /// Frame loss and success, always None.
        #[serde(skip)]
        pub frame: Option<()>,
    }

    /// Statistics about a node and its host.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Stats {
        /// The opcode of the event.
        pub op: Opcode,
        /// The current number of total players (active and not active) within
        /// the node.
        pub players: u64,
        /// The current number of active players within the node.
        pub playing_players: u64,
        /// The uptime of the Lavalink server in seconds.
        pub uptime: u64,
        /// Memory information about the node's host.
        pub memory: StatsMemory,
        /// CPU information about the node's host.
        pub cpu: StatsCpu,
        /// Statistics about audio frames.
        #[serde(rename = "frameStats", skip_serializing_if = "Option::is_none")]
        pub frames: Option<StatsFrames>,
    }

    /// Memory information about a node and its host.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StatsMemory {
        /// The number of bytes allocated.
        pub allocated: u64,
        /// The number of bytes free.
        pub free: u64,
        /// The number of bytes reservable.
        pub reservable: u64,
        /// The number of bytes used.
        pub used: u64,
    }

    /// CPU information about a node and its host.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StatsCpu {
        /// The number of CPU cores.
        pub cores: usize,
        /// The load of the Lavalink server.
        pub lavalink_load: f64,
        /// The load of the system as a whole.
        pub system_load: f64,
    }

    /// Frame statistics.
    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StatsFrames {
        /// Frames sent per minute.
        pub sent: i64,
        /// Frames nulled per minute.
        pub nulled: i64,
        /// Frames deficit per minute.
        pub deficit: i64,
    }

    /// The type of track event that was received.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub enum TrackEventType {
        /// A track for a player started.
        #[serde(rename = "TrackStartEvent")]
        Start,
        /// A track for a player ended.
        #[serde(rename = "TrackEndEvent")]
        End,
        /// A track for a player met an exception.
        #[serde(rename = "TrackExceptionEvent")]
        Exception,
        /// A track for a player got stuck.
        #[serde(rename = "TrackStuckEvent")]
        Stuck,
        /// A websocket got closed.
        #[serde(rename = "WebSocketClosedEvent")]
        WebsocketClose,
        /// A player got destroyed.
        #[serde(rename = "PlayerDestroyedEvent")]
        PlayerDestroy,
    }

    /// A track started.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TrackStart {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The base64 track that was affected.
        pub track: String,
    }

    /// A track ended.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TrackEnd {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The base64 track that was affected.
        pub track: String,
        /// The reason that the track ended.
        pub reason: String,
    }

    /// A track encountered exception.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TrackException {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The base64 track that was affected.
        pub track: String,
        /// The error that the track encountered exception.
        pub error: String,
        /// The specific error.
        pub exception: Error,
    }

    /// A track got stuck.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TrackStuck {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The base64 track that was affected.
        pub track: String,
        /// The threshold for track stuck.
        pub threshold_ms: i64,
    }

    /// A websocket got closed.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WebsocketClose {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// The reason for the close of websocket.
        pub reason: Option<String>,
        /// The code for this websocket close.
        pub code: i64,
        /// Whether it is closed by remote.
        pub by_remote: bool,
    }

    /// A player got destroyed.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PlayerDestroy {
        /// The opcode of the event.
        pub op: Opcode,
        /// The type of track event.
        #[serde(rename = "type")]
        pub kind: TrackEventType,
        /// The guild ID of the player.
        pub guild_id: GuildId,
        /// The user ID affected, always None.
        #[serde(skip)]
        pub user_id: Option<()>,
        /// Whether player is destroyed during cleanup.
        pub cleanup: bool,
    }
}

pub use self::{
    incoming::{
        IncomingEvent, PlayerDestroy, PlayerUpdate, PlayerUpdateState, Stats, StatsCpu,
        StatsFrames, StatsMemory, TrackEnd, TrackEventType, TrackException, TrackStart, TrackStuck,
        WebsocketClose,
    },
    outgoing::{
        Destroy, Equalizer, Filters, GetPlayer, Karaoke, OutgoingEvent, Play,
        SlimVoiceServerUpdate, Stop, Timescale, Tremolo, Update, Vibrato, VoiceUpdate,
    },
};
