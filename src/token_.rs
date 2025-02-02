use core::{
    borrow::Borrow,
    future::{Future, IntoFuture},
    pin::Pin,
    ptr,
    task::{Context, Poll},
};

use pin_project::pin_project;
use pin_utils::pin_mut;

use abs_sync::{
    cancellation::TrCancellationToken,
    x_deps::pin_utils,
};
use atomex::TrCmpxchOrderings;

use snapshot_channel::{
    x_deps::{abs_sync, atomex},
    Glimpse, Snapshot,
};

pub struct CancellationToken<B, F, O>(Snapshot<B, F, O>)
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings;

impl<B, F, O> CancellationToken<B, F, O>
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    pub const fn new(snapshot: Snapshot<B, F, O>) -> Self {
        CancellationToken(snapshot)
    }

    pub fn cancellation(
        self: Pin<&mut Self>,
    ) -> CancellationAsync<'_, B, F, O> {
        CancellationAsync(unsafe {
            let pointer = &mut self.get_unchecked_mut().0;
            Pin::new_unchecked(pointer)
        })
    }
}

impl<B, F, O> Clone for CancellationToken<B, F, O>
where
    B: Clone + Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

impl<B, F, O> TrCancellationToken for CancellationToken<B, F, O>
where
    B: Clone + Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    type Cancellation<'a> = CancellationAsync<'a, B, F, O> where Self: 'a;

    fn is_cancelled(&self) -> bool {
        self.0.is_ready()
    }

    fn can_be_cancelled(&self) -> bool {
        !self.0.is_ready()
    }

    fn cancellation(self: Pin<&mut Self>) -> Self::Cancellation<'_> {
        CancellationToken::cancellation(self)
    }
}

impl<B, F, O> PartialEq for CancellationToken<B, F, O>
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.0.glimpse(), other.0.glimpse())
    }
}

unsafe impl<B, F, O> Send for CancellationToken<B, F, O>
where
    B: Borrow<Glimpse<F, O>> + Send,
    F: Future,
    O: TrCmpxchOrderings,
{}

unsafe impl<B, F, O> Sync for CancellationToken<B, F, O>
where
    B: Borrow<Glimpse<F, O>> + Sync,
    F: Future,
    O: TrCmpxchOrderings,
{}

pub struct CancellationAsync<'a, B, F, O>(Pin<&'a mut Snapshot<B, F, O>>)
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings;

impl<'a, B, F, O> IntoFuture for CancellationAsync<'a, B, F, O>
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    type IntoFuture = CancellationFuture<'a, B, F, O>;
    type Output = <Self::IntoFuture as Future>::Output;

    fn into_future(self) -> Self::IntoFuture {
        CancellationFuture(self.0)
    }
}

#[pin_project]
pub struct CancellationFuture<'a, B, F, O>(Pin<&'a mut Snapshot<B, F, O>>)
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings;

impl<B, F, O> Future for CancellationFuture<'_, B, F, O>
where
    B: Borrow<Glimpse<F, O>>,
    F: Future,
    O: TrCmpxchOrderings,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let peek = this.0.as_mut().peek_async().into_future();
        pin_mut!(peek);
        peek.poll(cx).map(|_| ())
    }
}