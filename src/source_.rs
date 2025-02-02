use core::{borrow::Borrow, future::Future};

use atomex::{StrictOrderings, TrCmpxchOrderings};
use snapshot_channel::{
    x_deps::atomex,
    Glimpse,
};

use crate::CancellationToken;

pub struct CancellationSource<F, O = StrictOrderings>(Glimpse<F, O>)
where
    F: Future,
    O: TrCmpxchOrderings;

impl<F, O> CancellationSource<F, O>
where
    F: Future,
    O: TrCmpxchOrderings,
{
    pub const fn new(future: F) -> Self {
        CancellationSource(Glimpse::new(future))
    }

    #[inline]
    pub fn is_cancellation_requested(&self) -> bool {
        self.0.try_peek().is_some()
    }

    #[inline]
    pub fn can_be_cancelled(&self) -> bool {
        self.0.try_peek().is_none()
    }

    pub fn child_token<B: Borrow<Self>>(

    ) -> CancellationToken<B, F, O> {
        CancellationToken::new(self.peeker_.borrow().clone())
    }
}
