use std::{collections::HashMap, io, net::SocketAddr};

use async_stream::stream;
use futures::Stream;
use iroha_actor::{broker::Broker, Actor, Addr, Context, ContextHandler, Handler};
use iroha_logger::{info, warn};
use parity_scale_codec::{Decode, Encode};
use tokio::net::{TcpListener, TcpStream};
use ursa::{encryption::symm::Encryptor, kex::KeyExchangeScheme};

use crate::{
    peer::{Peer, PeerId, State},
    Error,
};

/// Main network layer structure, that is holding connections, called [`Peer`]s.
#[derive(Debug)]
pub struct Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    peers: HashMap<PeerId, Addr<Peer<T, K, E>>>,
    listener: Option<TcpListener>,
    broker: Broker,
}

impl<T, K, E> Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    /// Creates a network structure, that will hold connections to other nodes.
    ///
    /// # Errors
    /// It will return Err if it is unable to start listening on specified address:port.
    pub async fn new(broker: Broker, listen_addr: String) -> Result<Self, Error> {
        let addr: SocketAddr = listen_addr.parse()?;
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            peers: HashMap::new(),
            listener: Some(listener),
            broker,
        })
    }

    /// Yields a stream of accepted peer connections.
    fn listener_stream(&mut self) -> impl Stream<Item = NewPeer> + Send + 'static {
        #[allow(clippy::expect_used)]
        let listener = self
            .listener
            .take()
            .expect("Unreachable, as it is supposed to have listener on the start");
        stream! {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        info!("Accepted connection from {}", &addr);
                        let id = PeerId { address: addr.to_string(), public_key: None };
                        let new_peer: NewPeer = NewPeer(Ok((stream, id)));
                        yield new_peer;
                    },
                    Err(error) => {
                        warn!(%error, "Error accepting connection");
                        yield NewPeer(Err(error));
                    }
                }
            }
        }
    }

    /// Starts an outgoing connection to other peer
    async fn connect_peer(&mut self, id: PeerId, addr: Addr<Network<T, K, E>>) {
        match Peer::new(id.clone(), None, State::Connecting, addr) {
            Ok(peer) => {
                let peer = peer.start().await;
                drop(self.peers.insert(id, peer));
            }
            Err(e) => {
                warn!(%e, "Unable to create peer");
            }
        }
    }
}

#[async_trait::async_trait]
impl<T, K, E> Actor for Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    async fn on_start(&mut self, ctx: &mut Context<Self>) {
        // from peer
        self.broker.subscribe::<Received<T>, _>(ctx);
        // from other iroha subsystems
        self.broker.subscribe::<Post<T>, _>(ctx);
        // register for peers from listener
        ctx.notify_with_context(self.listener_stream());
    }
}

#[async_trait::async_trait]
impl<T, K, E> ContextHandler<Connect> for Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    type Result = ();

    async fn handle(&mut self, ctx: &mut Context<Self>, msg: Connect) {
        let addr = ctx.addr();
        self.connect_peer(msg.id, addr).await;
    }
}

#[async_trait::async_trait]
impl<T, K, E> Handler<Post<T>> for Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    type Result = ();

    async fn handle(&mut self, msg: Post<T>) {
        let addr: &Addr<Peer<T, K, E>> = &self.peers[&msg.id];
        addr.do_send(msg).await;
    }
}

#[async_trait::async_trait]
impl<T, K, E> Handler<Received<T>> for Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    type Result = ();

    async fn handle(&mut self, _msg: Received<T>) {
        // TODO send message to Torii
        //self.handler.do_send(msg.message);
    }
}

#[async_trait::async_trait]
impl<T, K, E> ContextHandler<NewPeer> for Network<T, K, E>
where
    T: Encode + Decode + Send + Clone + 'static,
    K: KeyExchangeScheme + Send + 'static,
    E: Encryptor + Send + 'static,
{
    type Result = ();

    async fn handle(&mut self, ctx: &mut Context<Self>, peer: NewPeer) {
        let addr = ctx.addr();
        let (stream, id) = match peer.0 {
            Ok(peer) => peer,
            Err(error) => {
                warn!(%error, "Error in listener!");
                return;
            }
        };
        match Peer::new(id.clone(), Some(stream), State::ConnectedFrom, addr) {
            Ok(peer) => {
                let peer = peer.start().await;
                drop(self.peers.insert(id, peer));
            }
            Err(e) => {
                warn!(%e, "Unable to create peer");
            }
        }
    }
}

/// The message that is sent to [Network] to start connection to some other peer.
#[derive(Clone, Debug, iroha_actor::Message)]
pub struct Connect {
    /// Peer identification
    pub id: PeerId,
}

/// The message received from other peer.
#[derive(Clone, Debug, iroha_actor::Message, Decode)]
pub struct Received<T: Encode + Decode> {
    /// Data received from another peer
    pub data: T,
    /// Peer identification
    pub id: PeerId,
}

/// The message to be sent to some other peer.
#[derive(Clone, Debug, iroha_actor::Message, Encode)]
pub struct Post<T: Encode> {
    /// Data to send to another peer
    pub data: T,
    /// Peer identification
    pub id: PeerId,
}

/// The result of some incoming peer connection.
#[derive(Debug, iroha_actor::Message)]
pub struct NewPeer(pub io::Result<(TcpStream, PeerId)>);