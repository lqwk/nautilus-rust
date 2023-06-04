use core::task::{Context, Poll};
use core::pin::Pin;
use crate::kernel::task::Future;

pub async fn yield_now() {
    struct YieldNow {
        polled_once: bool,
    }

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.polled_once {
                Poll::Ready(())
            } else {
                self.polled_once = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    YieldNow { polled_once: false }.await
}
