//! Scope Windows timer precision to active transitions, never the idle app.
//!
//! A nominal 16 ms channel timeout can round to ~31 ms at the default Windows
//! timer resolution. This matching begin/end pair avoids that quantization.
//! https://learn.microsoft.com/windows/win32/api/timeapi/nf-timeapi-timebeginperiod

pub struct AnimationTimerResolution {
    #[cfg(windows)]
    acquired: bool,
}

impl AnimationTimerResolution {
    pub fn acquire() -> Self {
        #[cfg(windows)]
        {
            // SAFETY: A fixed, supported millisecond period is passed without
            // pointers. Drop matches every successful request, including when
            // the region worker exits during an interrupted transition.
            let acquired = unsafe { windows_sys::Win32::Media::timeBeginPeriod(1) } == 0;
            if !acquired {
                eprintln!("Zone could not acquire active-animation timer precision");
            }
            Self { acquired }
        }
        #[cfg(not(windows))]
        Self {}
    }
}

impl Drop for AnimationTimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.acquired {
            // SAFETY: Matches the successful timeBeginPeriod(1) owned by self.
            let result = unsafe { windows_sys::Win32::Media::timeEndPeriod(1) };
            if result != 0 {
                eprintln!("Zone could not release active-animation timer precision");
            }
        }
    }
}
