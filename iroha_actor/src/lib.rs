//!
//! Iroha simple actor framework.
//!

use std::any::type_name;

use async_std::{
    sync as oneshot,
    sync::{self as mpsc, RecvError},
};
use dev::*;
use envelope::{Envelope, SyncEnvelopeProxy, ToEnvelope};
#[cfg(feature = "deadlock_detection")]
use {async_std::task, deadlock::ActorId};

pub mod broker;
#[cfg(feature = "deadlock_detection")]
mod deadlock;
mod envelope;

pub mod prelude {
    //! Module with most used items
    pub use super::{dev::Context, Actor, Addr, Handler, Message, Recipient};
}

/// Address of actor. Can be used to send messages to it.
#[derive(Debug)]
pub struct Addr<A: Actor> {
    sender: mpsc::Sender<Envelope<A>>,
    #[cfg(feature = "deadlock_detection")]
    actor_id: ActorId,
}

impl<A: Actor> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            #[cfg(feature = "deadlock_detection")]
            actor_id: self.actor_id,
        }
    }
}

impl<A: Actor> Addr<A> {
    fn new(sender: mpsc::Sender<Envelope<A>>) -> Self {
        Self {
            sender,
            #[cfg(feature = "deadlock_detection")]
            actor_id: ActorId::from(&task::current()),
        }
    }

    /// Send a message and wait for an answer.
    /// # Errors
    /// Fails if noone will send message
    pub async fn send<M>(&self, message: M) -> Result<M::Result, RecvError>
    where
        M: Message + Send + 'static,
        M::Result: Send,
        A: Handler<M>,
    {
        let (sender, reciever) = oneshot::channel(1);
        let envelope = SyncEnvelopeProxy::pack(message, Some(sender));
        #[cfg(feature = "deadlock_detection")]
        deadlock::r#in(self.actor_id).await;
        self.sender.send(envelope).await;
        let result = reciever.recv().await;
        #[cfg(feature = "deadlock_detection")]
        deadlock::out(self.actor_id).await;
        result
    }

    /// Send a message and wait for an answer.
    /// # Errors
    /// Fails if queue is full or actor is disconnected
    #[allow(clippy::result_unit_err)]
    pub async fn do_send<M>(&self, message: M)
    where
        M: Message + Send + 'static,
        M::Result: Send,
        A: Handler<M>,
    {
        let envelope = SyncEnvelopeProxy::pack(message, None);
        self.sender.send(envelope).await
    }

    /// Constructs recipient for sending only specific messages (without answers)
    pub fn recipient<M>(&self) -> Recipient<M>
    where
        M: Message<Result = ()> + Send + 'static,
        A: Handler<M>,
    {
        Recipient(Box::new(self.clone()))
    }
}

#[allow(missing_debug_implementations)]
/// Address of actor. Can be used to send messages to it.
pub struct Recipient<M: Message<Result = ()>>(Box<dyn Sender<M> + Sync + Send + 'static>);

impl<M: Message<Result = ()> + Send> Recipient<M> {
    /// Send message to actor
    pub async fn send(&self, m: M) {
        self.0.send(m).await
    }
}

#[async_trait::async_trait]
trait Sender<M: Message<Result = ()>> {
    async fn send(&self, m: M);
}

#[async_trait::async_trait]
impl<A, M> Sender<M> for Addr<A>
where
    M: Message<Result = ()> + Send + 'static,
    M::Result: Send,
    A: Handler<M>,
{
    async fn send(&self, m: M) {
        self.do_send(m).await
    }
}

/// Actor trait
#[async_trait::async_trait]
pub trait Actor: Send + Sized + 'static {
    /// Capacity of actor queue
    const CAPACITY: usize = 100;

    /// At start hook of actor
    async fn started(&mut self, _ctx: &mut Context<Self>) {}

    /// At stop hook of actor
    async fn stopped(&mut self, _ctx: &mut Context<Self>) {}

    /// Start actor
    fn start(mut self) -> Addr<Self> {
        let (sender, reciever) = mpsc::channel(Self::CAPACITY);
        let addr = Addr::new(sender);
        let mut ctx = Context::new(addr.clone());
        #[allow(clippy::expect_used)]
        let join_handle = task::spawn(async move {
            self.started(&mut ctx).await;
            while let Ok(Envelope(mut message)) = reciever.recv().await {
                message.handle(&mut self, &mut ctx).await;
            }
            self.stopped(&mut ctx).await;
        });
        #[cfg(feature = "deadlock_detection")]
        {
            let mut addr = addr;
            addr.actor_id = ActorId {
                id: join_handle.task().id(),
                name: Some(type_name::<Self>()),
            };
            addr
        }
        #[cfg(not(feature = "deadlock_detection"))]
        addr
    }

    /// Start actor with default values
    fn start_default() -> Addr<Self>
    where
        Self: Default,
    {
        Self::default().start()
    }
}

/// Message trait for setting result of message
pub trait Message {
    /// Result type of message
    type Result: 'static;
}

/// Trait for actor for handling specific message type
#[async_trait::async_trait]
pub trait Handler<M: Message>: Actor {
    /// Result of handler
    type Result: MessageResponse<M>;

    /// Message handler
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: M) -> Self::Result;
}

pub mod dev {
    //! Module with development types

    use super::*;

    /// Dev trait for Message responding
    #[async_trait::async_trait]
    pub trait MessageResponse<M: Message>: Send {
        /// Handles message
        async fn handle(self, sender: oneshot::Sender<M::Result>);
    }

    #[async_trait::async_trait]
    impl<M: Message<Result = ()>> MessageResponse<M> for () {
        async fn handle(self, sender: oneshot::Sender<M::Result>) {
            let _ = sender.send(()).await;
        }
    }

    /// Context for execution of actor
    #[derive(Debug)]
    pub struct Context<A: Actor> {
        addr: Addr<A>,
    }

    impl<A: Actor> Context<A> {
        /// Default constructor
        pub fn new(addr: Addr<A>) -> Self {
            Self { addr }
        }

        /// Gets an address of current actor
        pub fn addr(&self) -> Addr<A> {
            self.addr.clone()
        }

        /// Gets an recipient for current actor with specified message type
        pub fn recipient<M>(&self) -> Recipient<M>
        where
            M: Message<Result = ()> + Send + 'static,
            A: Handler<M>,
        {
            self.addr().recipient()
        }
    }
}
