use std::cell::Cell;

pub trait TerminalOps {
    fn enter(&self) -> std::io::Result<()>;
    fn restore(&self);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RatatuiOps;

impl TerminalOps for RatatuiOps {
    fn enter(&self) -> std::io::Result<()> {
        Ok(())
    }
    fn restore(&self) {
        ratatui::restore();
    }
}

pub struct TerminalGuard<O: TerminalOps> {
    ops: O,
    armed: Cell<bool>,
}

impl<O: TerminalOps> TerminalGuard<O> {
    pub fn enter(ops: O) -> std::io::Result<Self> {
        ops.enter()?;
        Ok(Self {
            ops,
            armed: Cell::new(true),
        })
    }

    pub fn restore_now(&self) {
        if self.armed.replace(false) {
            self.ops.restore();
        }
    }
}

impl<O: TerminalOps> Drop for TerminalGuard<O> {
    fn drop(&mut self) {
        self.restore_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone)]
    struct FakeOps(Arc<AtomicUsize>);
    impl TerminalOps for FakeOps {
        fn enter(&self) -> std::io::Result<()> {
            Ok(())
        }
        fn restore(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn ut_110_guard_restores_on_drop_unwind_and_signal_path() {
        let drops = Arc::new(AtomicUsize::new(0));
        {
            let _guard = TerminalGuard::enter(FakeOps(drops.clone())).unwrap();
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        let unwind = std::panic::catch_unwind({
            let drops = drops.clone();
            move || {
                let _guard = TerminalGuard::enter(FakeOps(drops)).unwrap();
                panic!("test");
            }
        });
        assert!(unwind.is_err());
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        let guard = TerminalGuard::enter(FakeOps(drops.clone())).unwrap();
        guard.restore_now();
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }
}
