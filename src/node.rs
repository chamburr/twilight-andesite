//! Nodes for communicating with a Lavalink server.
//!
//! Using nodes, you can send events to a server and receive events.
//!
//! This is a bit more low level than using the [`Lavalink`] client because you
//! will need to provide your own `VoiceUpdate` events when your bot joins
//! channels, meaning you will have to accumulate and combine voice state update
//! and voice server update events from the Discord gateway to send them to
//! a node.
//!
//! Additionally, you will have to create and manage your own [`PlayerManager`]
//! and make your own players for guilds when your bot joins voice channels.
//!
//! This can be a lot of work, and there's not really much reason to do it
//! yourself. For that reason, you should almost always use the `Lavalink`
//! client which does all of this for you.
//!
//! [`Lavalink`]: ../client/struct.Lavalink.html
//! [`PlayerManager`]: ../player/struct.PlayerManager.html

use crate::{
    model::{IncomingEvent, Opcode, OutgoingEvent, PlayerUpdate, Stats, StatsCpu, StatsMemory},
    player::PlayerManager,
};
use async_tungstenite::{
    tokio::ConnectStream,
    tungstenite::{Error as TungsteniteError, Message},
    WebSocketStream,
};
use futures_channel::mpsc::{self, TrySendError, UnboundedReceiver, UnboundedSender};
use futures_util::{
    future::{self, Either},
    lock::BiLock,
    sink::SinkExt,
    stream::StreamExt,
};
use http::{
    header::{ToStrError, AUTHORIZATION, CONNECTION, UPGRADE},
    Error as HttpError, Request, Response, StatusCode,
};
use reqwest::{Client, Error as ReqwestError};
use serde_json::Error as JsonError;
use std::{
    convert::TryInto,
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
    net::SocketAddr,
    num::ParseIntError,
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;
use twilight_model::id::UserId;

/// An error occurred while either initializing a connection or while running
/// its event loop.
#[derive(Debug)]
pub enum NodeError {
    /// Building the HTTP request to initialize a connection failed.
    BuildingConnectionRequest {
        /// The source of the error from the `http` crate.
        source: HttpError,
    },
    /// Error executing a HTTP request.
    ExecutingRequest {
        /// The source of the error from the `reqwest` crate.
        source: ReqwestError,
    },
    /// Error parsing a HTTP response header.
    ParsingResponseHeader {
        /// The source of the error from the `http` crate.
        source: ToStrError,
    },
    /// Error parsing a string to an integer.
    ParsingInt {
        /// The source of the error from `std`.
        source: ParseIntError,
    },
    /// Connecting to the Lavalink server failed after several backoff attempts.
    Connecting {
        /// The source of the error from the `tungstenite` crate.
        source: TungsteniteError,
    },
    /// Serializing a JSON message to be sent to a Lavalink node failed.
    SerializingMessage {
        /// The message that couldn't be serialized.
        message: OutgoingEvent,
        /// The source of the error from the `serde_json` crate.
        source: JsonError,
    },
    /// The given authorization for the node is incorrect.
    Unauthorized {
        /// The address of the node that failed to authorize.
        address: SocketAddr,
        /// The authorization used to connect to the node.
        authorization: String,
    },
}

impl Display for NodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::BuildingConnectionRequest { .. } => {
                f.write_str("failed to build connection request")
            }
            Self::ExecutingRequest { .. } => f.write_str("failed to execute http request"),
            Self::ParsingResponseHeader { .. } => f.write_str("failed to parse response header"),
            Self::ParsingInt { .. } => f.write_str("failed to parse string to int"),
            Self::Connecting { .. } => f.write_str("failed to connect to the node"),
            Self::SerializingMessage { .. } => {
                f.write_str("failed to serialize outgoing message as json")
            }
            Self::Unauthorized { address, .. } => write!(
                f,
                "the authorization used to connect to node {} is invalid",
                address
            ),
        }
    }
}

impl Error for NodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BuildingConnectionRequest { source } => Some(source),
            Self::ExecutingRequest { source } => Some(source),
            Self::ParsingResponseHeader { source } => Some(source),
            Self::ParsingInt { source } => Some(source),
            Self::Connecting { source } => Some(source),
            Self::SerializingMessage { source, .. } => Some(source),
            Self::Unauthorized { .. } => None,
        }
    }
}

/// The configuration that a [`Node`] uses to connect to a Lavalink server.
///
/// [`Node`]: struct.Node.html
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeConfig {
    /// The user ID of the bot.
    pub user_id: UserId,
    /// The address of the node.
    pub address: SocketAddr,
    /// The password to use when authenticating.
    pub authorization: String,
    /// The details for resuming a Lavalink session, if any.
    ///
    /// Set this to `None` to disable resume capability.
    pub resume: Option<Resume>,
}

/// Configuration for a session which can be resumed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resume {
    /// The number of milliseconds that the Lavalink server will allow the
    /// session to be resumed for after a disconnect.
    pub timeout: u64,
    /// The connection id to resume as. Set to None to disable initial resume.
    pub connection_id: Option<u64>,
}

impl Resume {
    /// Configure resume capability, providing the number of seconds that the
    /// Lavalink server should queue events for when the connection is resumed.
    pub fn new(timeout: u64) -> Self {
        Self::new_with_id(timeout, None)
    }

    /// Similar to [`new`], but allows you to specify connection id.
    ///
    /// [`new`]: #method.new
    pub fn new_with_id(timeout: u64, id: impl Into<Option<u64>>) -> Self {
        Self {
            timeout,
            connection_id: id.into(),
        }
    }
}

impl NodeConfig {
    /// Create a new configuration for connecting to a node via
    /// [`Node::connect`].
    ///
    /// If adding a node through the [`Lavalink`] client then you don't need to
    /// do this yourself.
    ///
    /// [`Lavalink`]: ../client/struct.Lavalink.html
    /// [`Node::connect`]: struct.Node.html#method.connect
    pub fn new(
        user_id: UserId,
        address: impl Into<SocketAddr>,
        authorization: impl Into<String>,
        resume: impl Into<Option<Resume>>,
    ) -> Self {
        Self {
            user_id,
            address: address.into(),
            authorization: authorization.into(),
            resume: resume.into(),
        }
    }
}

#[derive(Debug)]
struct NodeRef {
    config: NodeConfig,
    lavalink_tx: UnboundedSender<OutgoingEvent>,
    players: PlayerManager,
    stats: BiLock<Stats>,
    connection_id: u64,
}

/// A connection to a single Lavalink server. It receives events and forwards
/// events from players to the server.
///
/// Please refer to the [module] documentation.
///
/// [module]: index.html
#[derive(Clone, Debug)]
pub struct Node(Arc<NodeRef>);

impl Node {
    /// Connect to a node, providing a player manager so that the node can
    /// update player details.
    ///
    /// Please refer to the [module] documentation for some additional
    /// information about directly creating and using nodes. You are encouraged
    /// to use the [`Lavalink`] client instead.
    ///
    /// [`Lavalink`]: ../client/struct.Lavalink.html
    /// [module]: index.html
    pub async fn connect(
        config: NodeConfig,
        players: PlayerManager,
    ) -> Result<(Self, UnboundedReceiver<IncomingEvent>), NodeError> {
        let (bilock_left, bilock_right) = BiLock::new(Stats {
            cpu: StatsCpu {
                cores: 0,
                lavalink_load: 0f64,
                system_load: 0f64,
            },
            frames: None,
            memory: StatsMemory {
                allocated: 0,
                free: 0,
                used: 0,
                reservable: 0,
            },
            players: 0,
            playing_players: 0,
            op: Opcode::Stats,
            uptime: 0,
        });

        let connection_id = {
            let mut req = http::Request::get(format!("http://{}", config.address));
            req = req.header(CONNECTION, "Upgrade");
            req = req.header(UPGRADE, "WebSocket");
            req = req.header(AUTHORIZATION, config.authorization.clone());
            req = req.header("User-Id", config.user_id.to_string());

            let req = req
                .body("")
                .map_err(|source| NodeError::BuildingConnectionRequest { source })?
                .try_into()
                .map_err(|source| NodeError::ExecutingRequest { source })?;
            let res = Client::new()
                .execute(req)
                .await
                .map_err(|source| NodeError::ExecutingRequest { source })?;

            let header_id = res.headers().get("andesite-connection-id");
            if let Some(id) = header_id {
                let id = id
                    .to_str()
                    .map_err(|source| NodeError::ParsingResponseHeader { source })?
                    .parse::<u64>()
                    .map_err(|source| NodeError::ParsingInt { source })?;
                id + 1
            } else {
                0
            }
        };

        tracing::debug!("starting connection to {}", config.address);
        let (conn_loop, lavalink_tx, lavalink_rx) =
            Connection::connect(config.clone(), players.clone(), bilock_right).await?;
        tracing::debug!("started connection to {}", config.address);

        let node = Self(Arc::new(NodeRef {
            config,
            lavalink_tx,
            players,
            stats: bilock_left,
            connection_id,
        }));

        tokio::spawn(conn_loop.run(node.clone()));

        Ok((node, lavalink_rx))
    }

    /// Retrieve an immutable reference to the node's configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.0.config
    }

    /// Retrieve an immutable reference to the player manager used by the node.
    pub fn players(&self) -> &PlayerManager {
        &self.0.players
    }

    /// Retrieve an immutable reference to the node's configuration.
    ///
    /// Note that sending player events through the node's sender won't update
    /// player states, such as whether it's paused.
    pub fn send(&self, event: impl Into<OutgoingEvent>) -> Result<(), TrySendError<OutgoingEvent>> {
        self.sender().unbounded_send(event.into())
    }

    /// Retrieve a unique sender to send events to the Lavalink server.
    ///
    /// Note that sending player events through the node's sender won't update
    /// player states, such as whether it's paused.
    pub fn sender(&self) -> UnboundedSender<OutgoingEvent> {
        self.0.lavalink_tx.clone()
    }

    /// Retrieve a copy of the node's stats.
    pub async fn stats(&self) -> Stats {
        (*self.0.stats.lock().await).clone()
    }

    /// Retrieve the connection id of the node.
    pub fn connection_id(&self) -> u64 {
        self.0.connection_id
    }

    /// Retrieve the calculated penalty score of the node.
    ///
    /// This score can be used to calculate how loaded the server is. A higher
    /// number means it is more heavily loaded.
    pub async fn penalty(&self) -> i32 {
        let stats = self.0.stats.lock().await;
        let cpu = 1.05f64.powf(100f64 * stats.cpu.system_load) * 10f64 - 10f64;

        let (deficit_frame, null_frame) = (
            1.03f64
                .powf(500f64 * (stats.frames.as_ref().map_or(0, |f| f.deficit) as f64 / 3000f64))
                * 300f64
                - 300f64,
            (1.03f64
                .powf(500f64 * (stats.frames.as_ref().map_or(0, |f| f.nulled) as f64 / 3000f64))
                * 300f64
                - 300f64)
                * 2f64,
        );

        stats.playing_players as i32 + cpu as i32 + deficit_frame as i32 + null_frame as i32
    }

    /// Provide a player update event.
    pub fn provide_player_update(
        &self,
        players: &PlayerManager,
        update: &PlayerUpdate,
    ) -> Result<(), NodeError> {
        if let Some(destroyed) = update.state.destroyed {
            if destroyed {
                return Ok(());
            }
        }

        let mut player = match players.get_mut(&update.guild_id) {
            Some(player) => player,
            None => players.get_or_insert(update.guild_id, self.clone()),
        };

        *player.value_mut().time_mut() = update.state.time;
        *player.value_mut().position_mut() = update.state.position;
        *player.value_mut().paused_mut() = update.state.paused;
        *player.value_mut().volume_mut() = update.state.volume;
        *player.value_mut().filters_mut() = update.state.filters.clone();

        Ok(())
    }
}

struct Connection {
    config: NodeConfig,
    connection: WebSocketStream<ConnectStream>,
    node_from: UnboundedReceiver<OutgoingEvent>,
    node_to: UnboundedSender<IncomingEvent>,
    players: PlayerManager,
    stats: BiLock<Stats>,
}

impl Connection {
    async fn connect(
        config: NodeConfig,
        players: PlayerManager,
        stats: BiLock<Stats>,
    ) -> Result<
        (
            Self,
            UnboundedSender<OutgoingEvent>,
            UnboundedReceiver<IncomingEvent>,
        ),
        NodeError,
    > {
        let connection = reconnect(&config).await?;

        let (to_node, from_lavalink) = mpsc::unbounded();
        let (to_lavalink, from_node) = mpsc::unbounded();

        Ok((
            Self {
                config,
                connection,
                node_from: from_node,
                node_to: to_node,
                players,
                stats,
            },
            to_lavalink,
            from_lavalink,
        ))
    }

    async fn run(mut self, node: Node) -> Result<(), NodeError> {
        loop {
            let from_lavalink = self.connection.next();
            let to_lavalink = self.node_from.next();

            match future::select(from_lavalink, to_lavalink).await {
                Either::Left((Some(Ok(incoming)), _)) => {
                    self.incoming(incoming, node.clone()).await?;
                }
                Either::Left((_, _)) => {
                    tracing::debug!("connection to {} closed, reconnecting", self.config.address);
                    self.connection = reconnect(&self.config).await?;
                }
                Either::Right((Some(outgoing), _)) => {
                    tracing::debug!(
                        "forwarding event to {}: {:?}",
                        self.config.address,
                        outgoing
                    );

                    let payload = serde_json::to_string(&outgoing).map_err(|source| {
                        NodeError::SerializingMessage {
                            message: outgoing,
                            source,
                        }
                    })?;

                    let msg = Message::Text(payload);
                    self.connection.send(msg).await.unwrap();
                }
                Either::Right((_, _)) => {
                    tracing::debug!("node {} closed, ending connection", self.config.address);

                    break;
                }
            }
        }

        Ok(())
    }

    async fn incoming(&mut self, incoming: Message, node: Node) -> Result<bool, NodeError> {
        tracing::debug!(
            "received message from {}: {:?}",
            self.config.address,
            incoming
        );

        let text = match incoming {
            Message::Close(_) => {
                tracing::debug!("got close, closing connection");
                let _ = self.connection.send(Message::Close(None)).await;

                return Ok(false);
            }
            Message::Ping(data) => {
                tracing::debug!("got ping, sending pong");
                let msg = Message::Pong(data);

                // We don't need to immediately care if a pong fails.
                let _ = self.connection.send(msg).await;

                return Ok(true);
            }
            Message::Text(text) => text,
            other => {
                tracing::debug!("got pong or bytes payload: {:?}", other);

                return Ok(true);
            }
        };

        let event = match serde_json::from_str(&text) {
            Ok(event) => event,
            Err(_) => {
                tracing::warn!("unknown message from lavalink node: {}", text);

                return Ok(true);
            }
        };

        match event {
            IncomingEvent::PlayerUpdate(ref update) => {
                self.player_update(update, node.clone()).await?;
            }
            IncomingEvent::PlayerDestroy(ref destroy) => {
                self.players.remove(&destroy.guild_id);
            }
            IncomingEvent::Stats(ref stats) => {
                self.stats(stats).await?;
            }
            _ => {}
        }

        // It's fine if the rx end dropped, often users don't need to care about
        // these events.
        if !self.node_to.is_closed() {
            let _ = self.node_to.unbounded_send(event);
        }

        Ok(true)
    }

    async fn player_update(&self, update: &PlayerUpdate, node: Node) -> Result<(), NodeError> {
        node.provide_player_update(&self.players, update)
    }

    async fn stats(&self, stats: &Stats) -> Result<(), NodeError> {
        *self.stats.lock().await = stats.clone();

        Ok(())
    }
}

fn connect_request(state: &NodeConfig) -> Result<Request<()>, NodeError> {
    let mut builder = Request::get(format!("ws://{}", state.address));
    builder = builder.header("Authorization", &state.authorization);
    builder = builder.header("User-Id", state.user_id.0);

    if let Some(resume) = state.resume.as_ref() {
        if let Some(connection_id) = resume.connection_id {
            builder = builder.header("Andesite-Resume-Id", connection_id.to_string());
        }
    }

    builder
        .body(())
        .map_err(|source| NodeError::BuildingConnectionRequest { source })
}

async fn reconnect(config: &NodeConfig) -> Result<WebSocketStream<ConnectStream>, NodeError> {
    let (mut stream, _) = backoff(config).await?;

    if let Some(resume) = config.resume.as_ref() {
        let payload = serde_json::json!({
            "op": "event-buffer",
            "timeout": resume.timeout,
        });
        let msg = Message::Text(serde_json::to_string(&payload).unwrap());

        stream.send(msg).await.unwrap();
    }

    Ok(stream)
}

async fn backoff(
    config: &NodeConfig,
) -> Result<(WebSocketStream<ConnectStream>, Response<()>), NodeError> {
    let mut seconds = 1;

    loop {
        let req = connect_request(config)?;

        match async_tungstenite::tokio::connect_async(req).await {
            Ok((stream, res)) => return Ok((stream, res)),
            Err(source) => {
                tracing::warn!("failed to connect to node {}: {:?}", source, config.address);

                if matches!(source, TungsteniteError::Http(ref res) if res.status() == StatusCode::UNAUTHORIZED)
                {
                    return Err(NodeError::Unauthorized {
                        address: config.address,
                        authorization: config.authorization.to_owned(),
                    });
                }

                if seconds > 64 {
                    tracing::debug!("no longer trying to connect to node {}", config.address);

                    return Err(NodeError::Connecting { source });
                }

                tracing::debug!(
                    "waiting {} seconds before attempting to connect to node {} again",
                    seconds,
                    config.address,
                );
                sleep(Duration::from_secs(seconds)).await;

                seconds *= 2;

                continue;
            }
        }
    }
}
