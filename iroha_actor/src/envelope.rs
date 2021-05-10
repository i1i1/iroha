#![allow(clippy::module_name_repetitions)]

use async_std::sync as oneshot;

use super::*;

pub trait ToEnvelope<A: Actor + Handler<M>, M: Message> {
    fn pack(msg: M, tx: Option<oneshot::Sender<M::Result>>) -> Envelope<A>;
}

pub struct Envelope<A: Actor>(pub Box<dyn EnvelopeProxy<A> + Send>);

impl<A, M> ToEnvelope<A, M> for SyncEnvelopeProxy<M>
where
    A: Actor + Handler<M>,
    M: Message + Send + 'static,
    M::Result: Send,
{
    fn pack(msg: M, tx: Option<oneshot::Sender<M::Result>>) -> Envelope<A> {
        Envelope(Box::new(Self { msg: Some(msg), tx }))
    }
}

#[async_trait::async_trait]
pub trait EnvelopeProxy<A: Actor> {
    async fn handle(&mut self, act: &mut A, ctx: &mut Context<A>);
}

pub struct SyncEnvelopeProxy<M>
where
    M: Message + Send,
    M::Result: Send,
{
    msg: Option<M>,
    tx: Option<oneshot::Sender<M::Result>>,
}

#[async_trait::async_trait]
impl<A, M> EnvelopeProxy<A> for SyncEnvelopeProxy<M>
where
    A: Actor + Handler<M> + Send,
    M: Message + Send,
    M::Result: Send,
{
    async fn handle(&mut self, act: &mut A, ctx: &mut Context<A>) {
        match (self.tx.take(), self.msg.take()) {
            (Some(tx), Some(msg)) => {
                let result = act.handle(ctx, msg).await;
                result.handle(tx).await;
            }
            (None, Some(msg)) => {
                let _ = act.handle(ctx, msg).await;
            }
            (_, None) => unreachable!(),
        }
    }
}
