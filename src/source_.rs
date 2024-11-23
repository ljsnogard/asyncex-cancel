use core::{
    borrow::BorrowMut,
    marker::PhantomData,
    ops::Deref,
    pin::Pin,
};

use abs_mm::mem_alloc::TrMalloc;
use atomex::{StrictOrderings, TrCmpxchOrderings};
use mm_ptr::{x_deps::abs_mm, Shared};
use spmv_oneshot::{
    x_deps::atomex,
    Oneshot, Peeker, Sender,
};
use crate::CancellationToken;

#[derive(Debug)]
pub struct CancellationTokenSource<BCh, BTx, BRx, O = StrictOrderings>
where
    BCh: Deref<Target = Oneshot<(), O>>,
    BTx: BorrowMut<Sender<BCh, (), O>>,
    BRx: BorrowMut<Peeker<BCh, (), O>>,
    O: TrCmpxchOrderings,
{
    _use_c_: PhantomData<BCh>,
    _use_o_: PhantomData<O>,
    sender_: Option<BTx>,
    peeker_: BRx,
}

impl<'a, O> CancellationTokenSource<
    Pin<&'a Oneshot<(), O>>,
    Sender<Pin<&'a Oneshot<(), O>>, (), O>,
    Peeker<Pin<&'a Oneshot<(), O>>, (), O>,
    O>
where
    O: TrCmpxchOrderings,
{
    pub fn from_oneshot(oneshot: Pin<&'a mut Oneshot<(), O>>) -> Self {
        let (tx, rx) = Oneshot::split(oneshot);
        let Result::Ok(peeker) = rx.try_into() else {
            unreachable!("[CancellationTokenSource::from_oneshot]")
        };
        Self {
            _use_c_: PhantomData,
            _use_o_: PhantomData,
            sender_: Option::Some(tx),
            peeker_: peeker,
        }
    }

    pub fn cancel(&mut self) -> bool {
        let Option::Some(mut sender) = self.sender_.take() else {
            return false;
        };
        sender
            .borrow_mut()
            .send(())
            .wait()
            .is_ok()
    }
}

impl<BCh, BTx, BRx, O> CancellationTokenSource<BCh, BTx, BRx, O>
where
    BCh: Deref<Target = Oneshot<(), O>>,
    BTx: BorrowMut<Sender<BCh, (), O>>,
    BRx: BorrowMut<Peeker<BCh, (), O>>,
    O: TrCmpxchOrderings,
{
    pub const fn new(sender: BTx, peeker: BRx) -> Self {
        Self {
            _use_c_: PhantomData,
            _use_o_: PhantomData,
            sender_: Option::Some(sender),
            peeker_: peeker,
        }
    }

    pub fn try_cancel(&self) -> bool {
        let Option::Some(sender) = &self.sender_ else {
            return false;
        };
        sender.borrow().send(()).wait().is_ok()
    }

    #[inline]
    pub fn is_cancellation_requested(&self) -> bool {
        self.peeker_.borrow().is_data_ready()
    }

    pub fn can_be_cancelled(&self) -> bool {
        let Option::Some(sender) = &self.sender_ else {
            return false;
        };
        sender.borrow().can_send()
    }
}

impl<BCh, BTx, BRx, O> CancellationTokenSource<BCh, BTx, BRx, O>
where
    BCh: Deref<Target = Oneshot<(), O>> + Clone,
    BTx: BorrowMut<Sender<BCh, (), O>>,
    BRx: BorrowMut<Peeker<BCh, (), O>>,
    O: TrCmpxchOrderings,
{
    pub fn child_token(&self) -> CancellationToken<BCh, O> {
        CancellationToken::new(self.peeker_.borrow().clone())
    }
}

impl<O, A> CancellationTokenSource<
    Shared<Oneshot<(), O>, A>,
    Sender<Shared<Oneshot<(), O>, A>, (), O>,
    Peeker<Shared<Oneshot<(), O>, A>, (), O>,
    O>
where
    O: TrCmpxchOrderings,
    A: TrMalloc + Clone,
{
    pub fn new_in(
        alloc: A,
        orderings: impl FnOnce() -> O,
    ) -> Self {
        let _ = orderings;
        let oneshot = Shared::new(Oneshot::new(), alloc);
        let try_split = Oneshot::try_split(oneshot, Shared::strong_count, Shared::weak_count);
        let Result::Ok((tx, rx)) = try_split else {
            unreachable!("[CancellationTokenSource::new_with_alloc] try_split_shared")
        };
        let Result::Ok(peeker) = rx.try_into() else {
            unreachable!("[CancellationTokenSource::new_with_alloc] try_into peeker")
        };
        Self::new(tx, peeker)
    }
}

impl<O, A> Default for CancellationTokenSource<
    Shared<Oneshot<(), O>, A>,
    Sender<Shared<Oneshot<(), O>, A>, (), O>,
    Peeker<Shared<Oneshot<(), O>, A>, (), O>,
    O>
where
    O: TrCmpxchOrderings + Default,
    A: TrMalloc + Clone + Default,
{
    fn default() -> Self {
        Self::new_in(A::default(), O::default)
    }
}
