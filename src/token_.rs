use core::{
    future::{self, Future, IntoFuture},
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

use pin_project::pin_project;
use pin_utils::pin_mut;

use abs_sync::{
    cancellation::TrCancellationToken,
    x_deps::pin_utils,
};
use atomex::TrCmpxchOrderings;

use asyncex_channel::{
    oneshot::{Peeker, Oneshot},
    x_deps::{abs_sync, atomex}
};

#[derive(Debug)]
pub struct CancellationToken<B, O>(Peeker<B, (), O>)
where
    B: Deref<Target = Oneshot<(), O>> + Clone,
    O: TrCmpxchOrderings;

impl<B, O> CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone,
    O: TrCmpxchOrderings,
{
    pub const fn new(peeker: Peeker<B, (), O>) -> Self {
        CancellationToken(peeker)
    }

    pub fn cancellation(self: Pin<&mut Self>) -> CancellationAsync<'_, B, O> {
        CancellationAsync(unsafe {
            let pointer = &mut self.get_unchecked_mut().0;
            Pin::new_unchecked(pointer)
        })
    }
}

impl<B, O> Clone for CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone,
    O: TrCmpxchOrderings,
{
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

impl<B, O> TrCancellationToken for CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone,
    O: TrCmpxchOrderings,
{
    fn is_cancelled(&self) -> bool {
        self.0.is_data_ready()
    }

    fn can_be_cancelled(&self) -> bool {
        !self.0.is_data_ready()
    }

    fn cancellation(self: Pin<&mut Self>) -> impl IntoFuture<Output = ()> {
        CancellationToken::cancellation(self)
    }
}

impl<B, O> PartialEq for CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone,
    O: TrCmpxchOrderings,
{
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.0, &other.0)
    }
}

unsafe impl<B, O> Send for CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone + Send,
    O: TrCmpxchOrderings,
{}

unsafe impl<B, O> Sync for CancellationToken<B, O>
where
    B: Deref<Target = Oneshot<(), O>> + Clone + Sync,
    O: TrCmpxchOrderings,
{}

pub struct CancellationAsync<'a, B, O>(Pin<&'a mut Peeker<B, (), O>>)
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings;

impl<'a, B, O> CancellationAsync<'a, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    pub fn orphan_as_unsignaled(self) -> OrphanAsUnsignaledFuture<'a, B, O> {
        OrphanAsUnsignaledFuture::new(self.0)
    }

    pub fn orphan_as_cancelled(self) -> OrphanAsCancelledFuture<'a, B, O> {
        OrphanAsCancelledFuture::new(self.0)
    }
}

impl<'a, B, O> IntoFuture for CancellationAsync<'a, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    type IntoFuture = OrphanAsCancelledFuture<'a, B, O>;
    type Output = <Self::IntoFuture as Future>::Output;

    #[cfg(feature = "orphan-tok-as-cancelled")]
    fn into_future(self) -> Self::IntoFuture {
        self.orphan_as_cancelled()
    }

    #[cfg(feature = "orphan-tok-as-unsignaled")]
    fn into_future(self) -> Self::IntoFuture {
        self.orphan_as_unsignaled()
    }
}

#[pin_project]
pub struct OrphanAsUnsignaledFuture<'a, B, O>(Pin<&'a mut Peeker<B, (), O>>)
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings;

impl<'a, B, O> OrphanAsUnsignaledFuture<'a, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    fn new(peeker: Pin<&'a mut Peeker<B, (), O>>) -> Self {
        OrphanAsUnsignaledFuture(peeker)
    }

    async fn get_signal_async_(self: Pin<&mut Self>) {
        let this = self.project();
        let peeker = this.0.as_mut();
        if peeker.peek_async().await.is_err() {
            future::pending::<()>().await;
            unreachable!()
        };
    }
}

impl<B, O> Future for OrphanAsUnsignaledFuture<'_, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f = self.get_signal_async_();
        pin_mut!(f);
        f.poll(cx)
    }
}

#[pin_project]
pub struct OrphanAsCancelledFuture<'a, B, O>(Pin<&'a mut Peeker<B, (), O>>)
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings;

impl<'a, B, O> OrphanAsCancelledFuture<'a, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    fn new(peeker: Pin<&'a mut Peeker<B, (), O>>) -> Self {
        OrphanAsCancelledFuture(peeker)
    }

    async fn get_signal_async_(self: Pin<&mut Self>) {
        let this = self.project();
        let peeker = this.0.as_mut();
        #[cfg(test)]
        log::trace!("[OrphanAsCancelledFuture::get_signal_async_] peek_async");

        #[allow(unused)]
        let Result::Err(e) = peeker.peek_async().await else {
            return;
        };
        #[cfg(test)]
        log::trace!("[OrphanAsCancelledFuture::get_signal_async_] orphaned {e:?}");
    }
}

impl<B, O> Future for OrphanAsCancelledFuture<'_, B, O>
where
    B: Deref<Target = Oneshot<(), O>>,
    O: TrCmpxchOrderings,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f = self.get_signal_async_();
        pin_mut!(f);
        f.poll(cx)
    }
}
