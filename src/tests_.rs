use std::{
    borrow::Borrow,
    boxed::Box,
    future::{self, IntoFuture},
    ops::Deref,
    ptr::NonNull,
    vec::*,
};
use pin_utils::pin_mut;

use abs_sync::{
    preludes::XtOkOr,
    x_deps::pin_utils,
    cancellation::TrCancellationToken,
};
use atomex::StrictOrderings;
use core_malloc::CoreAlloc as TestAlloc;
use mm_ptr::Shared;

use snapshot_channel::{x_deps::{abs_sync, atomex}, *};

use super::*;

type Cts = CancellationSource::<
    Shared<Oneshot<(), StrictOrderings>, TestAlloc>,
    Sender<Shared<Oneshot<(), StrictOrderings>, TestAlloc>, (), StrictOrderings>,
    Peeker<Shared<Oneshot<(), StrictOrderings>, TestAlloc>, (), StrictOrderings>,
    StrictOrderings,
>;
type CancelToken = CancellationToken<
    Shared<Oneshot<(), StrictOrderings>, TestAlloc>,
    StrictOrderings,
>;

#[test]
fn default_cancellation_token_source_should_be_cancellable() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    assert!(cts.can_be_cancelled());
}

#[test]
fn default_cancellation_token_source_child_token_should_be_cancellable() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    assert!(!tok.is_cancelled())
}

#[test]
fn orphaned_cancellation_token_source_should_be_cancellable() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    assert!(cts.can_be_cancelled());
    assert!(cts.try_cancel());
}

#[test]
fn cancellation_token_source_should_signal_all_child_tokens() {
    const CHILDREN_COUNT: usize = 64;
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };

    assert!(cts.can_be_cancelled());
    assert!(!cts.is_cancellation_requested());

    let tokens: Vec<CancelToken> = (0..CHILDREN_COUNT)
        .map(|_| {
            let tok = cts.child_token();
            assert!(tok.can_be_cancelled());
            assert!(!tok.is_cancelled());
            tok
        })
        .collect();
    assert!(cts.try_cancel());
    tokens.iter().for_each(|tok| {
        assert!(!tok.can_be_cancelled());
        assert!(tok.is_cancelled());
    });
}

#[tokio::test]
async fn cancellation_await_should_work() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    let tok_cloned = tok.clone();

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());

    pin_mut!(tok_cloned);
    let tok_cancel_fut = tok_cloned.cancellation();
    assert!(cts.try_cancel());

    assert!(tok.is_cancelled());
    assert!(!tok.can_be_cancelled());

    tok_cancel_fut.await;
}

#[tokio::test]
async fn spawned_cancellation_await_should_work() {
    let _ = env_logger::builder().is_test(true).try_init();

    const CHILDREN_COUNT: usize = 4;
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };

    assert!(cts.can_be_cancelled());
    assert!(!cts.is_cancellation_requested());

    let mut tokens: Vec<Box<CancellationToken<_, StrictOrderings>>> = (0..CHILDREN_COUNT)
        .map(|_| {
            let tok = cts.child_token();
            assert!(tok.can_be_cancelled());
            assert!(!tok.is_cancelled());
            Box::new(tok)
        })
        .collect();
    let mut handles = Vec::<tokio::task::JoinHandle<()>>::new();
    for tok in tokens.iter_mut() {
        log::trace!("[spawned_cancellation_await_should_work] before spawn");
        let mut tok = unsafe { NonNull::new_unchecked(tok.as_mut()) };
        let handle = tokio::task::spawn(unsafe {
            tok.as_mut()
                .cancellation()
                .cancel_on_orphaned()
        });
        handles.push(handle);
    };
    assert!(cts.try_cancel());
    for handle in handles.into_iter() {
        assert!(handle.await.is_ok())
    }
    drop(tokens);
}

#[tokio::test]
async fn cts_drop_should_signal_cancel_token() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    pin_mut!(tok);

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    drop(cts);
    assert!(
        future::pending::<()>()
            .ok_or(tok.cancellation().cancel_on_orphaned())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn ok_or_future_should_work_for_pending_future() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    pin_mut!(tok);

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    let pending = future::pending::<()>();
    let f = pending.ok_or(tok.cancellation().into_future());

    assert!(cts.try_cancel());
    assert!(f.await.is_err());
}

#[tokio::test]
async fn ok_or_should_work_for_ready_future() {
    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    pin_mut!(tok);

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    let ready = future::ready::<()>(());
    pin_mut!(ready);
    let async_result_fut = ready.ok_or(tok.cancellation().into_future());
    assert!(async_result_fut.await.is_ok());
}

/// 使用 `async_channel::unbounded` 来测试 `CancellationToken` 和 `OkOr` 的功能，
/// 模拟任意 `Future` (`async_channel::Receiver`) 在等待过程中接收到来自
/// `CancellationToken` 因为被废弃而发出的取消信号
#[tokio::test]
async fn ok_or_cancelled_smoke() {
    let _ = env_logger::builder().is_test(true).try_init();

    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();
    let (spawn_tx, spawn_rx) = async_channel::unbounded::<()>();
    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    let (tx, rx) = async_channel::unbounded::<usize>();
    let recv_task = tokio::task::spawn(async move {
        pin_mut!(tok);
        assert!(!tok.is_cancelled());
        assert!(spawn_tx.send(()).await.is_ok());
        log::trace!("[ok_or_cancelled_smoke] rx.recv()");
        let r = rx
            .recv()
            .ok_or(tok.as_mut().cancellation().cancel_on_orphaned())
            .await;
        log::trace!(
            "[ok_or_cancelled_smoke] tok.is_cancelled({}), r.is_err({})",
            tok.is_cancelled(),
            r.is_err()
        );
        r
    });
    assert!(spawn_rx.recv().await.is_ok());
    log::trace!("[ok_or_cancelled_smoke] cts will drop");
    drop(cts);
    let received = recv_task.await;
    assert!(matches!(received, Result::Ok(Result::Err(_))));
    drop(tx);
}

#[tokio::test]
async fn ok_or_unsignaled_orphaned_smoke() {
    let _ = env_logger::builder().is_test(true).try_init();

    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    let (tx, rx) = async_channel::unbounded::<usize>();
    let (spawn_tx, spawn_rx) = async_channel::unbounded::<()>();
    let recv_task = tokio::task::spawn(async move {
        pin_mut!(tok);
        assert!(!tok.is_cancelled());
        assert!(spawn_tx.send(()).await.is_ok());
        let r = rx
            .recv()
            .ok_or(tok.as_mut().cancellation().pend_on_orphaned())
            .await;
        log::trace!(
            "[ok_or_unsignaled_orphaned_smoke] tok.is_cancelled({}), r.is_err({})",
            tok.is_cancelled(),
            r.is_err()
        );
        r
    });
    assert!(spawn_rx.recv().await.is_ok());
    log::trace!("[ok_or_unsignaled_orphaned_smoke] cts will drop");
    drop(cts);
    assert!(tx.send(42).await.is_ok());
    assert!(recv_task.await.is_ok());
}

#[tokio::test]
async fn ok_or_cancelled_orphaned_smoke() {
    let _ = env_logger::builder().is_test(true).try_init();

    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let tok = cts.child_token();

    assert!(cts.can_be_cancelled());
    assert!(tok.can_be_cancelled());
    assert!(!tok.is_cancelled());

    let (tx, rx) = async_channel::unbounded::<usize>();
    let recv_task = tokio::task::spawn(async move {
        pin_mut!(tok);
        assert!(!tok.is_cancelled());
        let r = rx
            .recv()
            .ok_or(tok.as_mut().cancellation().cancel_on_orphaned())
            .await;
        log::trace!(
            "[ok_or_cancelled_orphaned_smoke] tok.is_cancelled({}), r.is_err({})",
            tok.is_cancelled(),
            r.is_err()
        );
        r
    });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    log::trace!("[ok_or_cancelled_orphaned_smoke] cts will drop");
    drop(cts);
    let recv = recv_task.await;
    assert!(matches!(recv, Result::Ok(Result::Err(_))));
    assert!(tx.send(42).await.is_err());
}

/// To verify if cancellation token will work with `Receiver::receive_async`
#[tokio::test]
async fn receive_async_cancel() {

    async fn recv_async<B, C>(
        rx: Receiver<B, (), StrictOrderings>,
        tok: impl Borrow<C>,
    ) -> Result<(), RxError<()>>
    where
        B: Deref<Target = Oneshot<(), StrictOrderings>>,
        C: Clone + TrCancellationToken,
    {
        pin_mut!(rx);
        let cancel = tok.borrow().clone();
        pin_mut!(cancel);
        rx.receive_async().may_cancel_with(cancel).await
    }

    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let oneshot = Shared::new(
        Oneshot::<(), StrictOrderings>::new(),
        TestAlloc::new(),
    );
    let Result::Ok((tx, rx)) = Oneshot
        ::try_split(oneshot, Shared::strong_count, Shared::weak_count)
    else {
        panic!("[tests_::receive_async_cancel] try_split failed");
    };
    let recv = tokio::task::spawn(recv_async(rx, cts.child_token()));
    cts.try_cancel();
    let x = recv.await;
    assert!(matches!(x, Result::Ok(Result::Err(RxError::Cancelled))));
    drop(tx)
}

/// To verify if cancellation token will work with `Peeker::peek_async`
#[tokio::test]
async fn peek_async_cancel() {

    async fn peek_async<B, C>(
        rx: Peeker<B, (), StrictOrderings>,
        tok: impl Borrow<C>,
    ) -> Result<(), RxError<()>>
    where
        B: Deref<Target = Oneshot<(), StrictOrderings>>,
        C: Clone + TrCancellationToken,
    {
        pin_mut!(rx);
        let cancel = tok.borrow().clone();
        pin_mut!(cancel);
        rx.peek_async()
            .may_cancel_with(cancel)
            .await
            .copied()
    }

    let Result::Ok(cts) = Cts::try_new(
        Shared::new(Oneshot::new(), TestAlloc::new()),
        Shared::strong_count,
        Shared::weak_count,
    ) else {
        unreachable!()
    };
    let oneshot = Shared::new(
        Oneshot::<(), StrictOrderings>::new(),
        TestAlloc::new(),
    );
    let Result::Ok((tx, rx)) = Oneshot
        ::try_split(oneshot, Shared::strong_count, Shared::weak_count)
    else {
        panic!("[tests_::receive_async_cancel] try_split failed");
    };
    let rx = rx.try_into().unwrap();
    let peek = tokio::task::spawn(peek_async(rx, cts.child_token()));
    cts.try_cancel();
    let x = peek.await;
    assert!(matches!(x, Result::Ok(Result::Err(RxError::Cancelled))));
    drop(tx)
}
