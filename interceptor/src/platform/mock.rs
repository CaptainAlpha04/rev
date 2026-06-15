use crate::Interceptor;
use rev_core::error::RevError;
use rev_core::types::{SyscallEvent, SyscallKind};
use std::time::{Duration, Instant, SystemTime};

pub struct MockInterceptor {
    next_event_id: u64,
    _start_time: Instant,
}

impl Default for MockInterceptor {
    fn default() -> Self {
        Self {
            next_event_id: 0,
            _start_time: Instant::now(),
        }
    }
}

impl MockInterceptor {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Interceptor for MockInterceptor {
    fn attach(&mut self, _pid: u32) -> Result<(), RevError> {
        self._start_time = Instant::now();
        Ok(())
    }

    fn next_event(&mut self) -> Result<SyscallEvent, RevError> {
        // Yield a few simulated events and then stop by indicating process exit.
        std::thread::sleep(Duration::from_millis(50));

        if self.next_event_id >= 5 {
            return Err(RevError::ReplayFailed {
                step: self.next_event_id,
                reason: "Target process exited (Mock)".to_string(),
            });
        }

        let timestamp_ns = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let (kind, return_bytes, fd) = match self.next_event_id {
            0 => (
                SyscallKind::ProcessId,
                12345u32.to_le_bytes().to_vec(),
                None,
            ),
            1 => (
                SyscallKind::TimeRead,
                vec![0; 16], // Mock clock_gettime timespec
                None,
            ),
            2 => (
                SyscallKind::RandomRead,
                vec![7, 7, 7, 7], // Lucky mock random numbers
                None,
            ),
            3 => (
                SyscallKind::FileRead {
                    path: Some("mock_settings.json".to_string()),
                },
                b"{\"mock\": true}".to_vec(),
                Some(3),
            ),
            _ => (SyscallKind::TimeRead, vec![0; 16], None),
        };

        let event = SyscallEvent {
            id: self.next_event_id,
            timestamp_ns,
            syscall: kind,
            return_bytes,
            fd,
        };

        self.next_event_id += 1;
        Ok(event)
    }

    fn detach(&mut self) -> Result<(), RevError> {
        Ok(())
    }
}
