use std::{cell::Cell, thread, time::Duration};

#[derive(Default)]
pub struct Waiter {
    is_active: Cell<bool>,
    time: Option<Duration>,
}

impl Waiter {
    pub fn from_secs(t: u64) -> Self {
        Self {
            is_active: Cell::new(false),
            time: Some(Duration::from_secs(t)),
        }
    }

    pub fn wait(&self) {
        if let Some(time) = self.time {
            if self.is_active.get() {
                thread::sleep(time);
            }
        }
        self.is_active.set(true);
    }
}

#[cfg(test)]
mod tests {
    use super::Waiter;

    #[test]
    fn wait() {
        let waiter = Waiter::from_secs(5);
        waiter.wait();
        let Waiter { is_active, .. } = waiter;
        assert!(is_active.into_inner());
    }
}
