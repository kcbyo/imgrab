use std::{cell::Cell, thread, time::Duration};

use crate::options::WaitOption;

#[derive(Default)]
pub struct Waiter {
    is_active: Cell<bool>,
    time: Option<Duration>,
}

impl Waiter {
    pub fn from_option(option: WaitOption) -> Self {
        let milliseconds = match option {
            WaitOption::Default => 1000,
            WaitOption::Specified(specified) => (1000.0 * specified) as u64,
        };

        Self {
            is_active: Cell::new(false),
            time: Some(Duration::from_millis(milliseconds)),
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
