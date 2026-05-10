use super::*;
use proptest::{collection::vec as pvec, prelude::*, test_runner::Config as ProptestConfig};
use std::{
    panic::{AssertUnwindSafe, RefUnwindSafe, UnwindSafe},
    rc::Rc,
};

static_assertions::assert_not_impl_any!(AppCell: Send, Sync);
static_assertions::assert_not_impl_any!(AppRef<'static>: Send, Sync);
static_assertions::assert_not_impl_any!(AppRefMut<'static>: Send, Sync);
static_assertions::assert_not_impl_any!(AppCell: UnwindSafe, RefUnwindSafe);

fn test_app_cell() -> Rc<AppCell> {
    crate::TestAppContext::single().app
}

fn is_app_borrowed<T>(result: Result<T, ReentryError>) -> bool {
    matches!(result, Err(ReentryError::AppBorrowed))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn prop_borrow_mut_then_borrow_mut_returns_app_borrowed_in_strict(repetitions in 1usize..32) {
        let app = test_app_cell();

        for _ in 0..repetitions {
            let guard = app.borrow_mut();

            prop_assert!(is_app_borrowed(app.try_borrow_mut()));
            prop_assert!(is_app_borrowed(app.try_borrow()));
            prop_assert_eq!(app.borrowed.get(), BorrowState::Mut);

            drop(guard);
            prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
        }
    }

    #[test]
    fn prop_drop_releases_borrow(kinds in pvec(any::<bool>(), 1..64)) {
        let app = test_app_cell();

        for shared in kinds {
            if shared {
                let guard = app.borrow();
                prop_assert!(is_app_borrowed(app.try_borrow_mut()));
                drop(guard);
            } else {
                let guard = app.borrow_mut();
                prop_assert!(is_app_borrowed(app.try_borrow_mut()));
                drop(guard);
            }

            prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
            prop_assert!(app.try_borrow_mut().is_ok());
            prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
        }
    }

    #[test]
    fn prop_panic_during_borrow_releases_borrow(repetitions in 1usize..16) {
        let app = test_app_cell();

        for _ in 0..repetitions {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let _guard = app.borrow_mut();
                panic!("intentional AppCell borrow panic");
            }));

            prop_assert!(result.is_err());
            prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
            prop_assert!(app.try_borrow_mut().is_ok());
            prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
        }
    }

    #[test]
    fn prop_borrow_share_count_caps(distance_from_cap in 0u32..3) {
        let app = test_app_cell();
        let count = u32::MAX - distance_from_cap;
        app.borrowed
            .set(BorrowState::Shared(NonZeroU32::new(count).unwrap()));

        if count == u32::MAX {
            prop_assert!(is_app_borrowed(app.try_borrow()));
            prop_assert_eq!(
                app.borrowed.get(),
                BorrowState::Shared(NonZeroU32::new(u32::MAX).unwrap())
            );
        } else {
            let guard = app.try_borrow().expect("shared borrow below cap should succeed");
            prop_assert_eq!(
                app.borrowed.get(),
                BorrowState::Shared(NonZeroU32::new(count + 1).unwrap())
            );
            drop(guard);
            prop_assert_eq!(
                app.borrowed.get(),
                BorrowState::Shared(NonZeroU32::new(count).unwrap())
            );
        }

        app.borrowed.set(BorrowState::Free);
        prop_assert!(app.try_borrow_mut().is_ok());
        prop_assert_eq!(app.borrowed.get(), BorrowState::Free);
    }
}
