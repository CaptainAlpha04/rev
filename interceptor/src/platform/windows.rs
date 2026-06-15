#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code, unused_imports, clippy::new_without_default, clippy::manual_c_str_literals)]

use crate::Interceptor;
use rev_core::error::RevError;
use rev_core::types::SyscallEvent;
use std::ffi::c_void;
use std::path::Path;

pub type HANDLE = *mut c_void;
pub type DWORD = u32;
pub type WORD = u16;
pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;
pub type SIZE_T = usize;
pub type BOOL = i32;

pub const INFINITE: DWORD = 0xFFFFFFFF;
pub const DBG_CONTINUE: DWORD = 0x00010002;
pub const DBG_EXCEPTION_NOT_HANDLED: DWORD = 0x80010001;

pub const EXCEPTION_DEBUG_EVENT: DWORD = 1;
pub const CREATE_THREAD_DEBUG_EVENT: DWORD = 2;
pub const CREATE_PROCESS_DEBUG_EVENT: DWORD = 3;
pub const EXIT_THREAD_DEBUG_EVENT: DWORD = 4;
pub const EXIT_PROCESS_DEBUG_EVENT: DWORD = 5;
pub const LOAD_DLL_DEBUG_EVENT: DWORD = 6;
pub const UNLOAD_DLL_DEBUG_EVENT: DWORD = 7;
pub const OUTPUT_DEBUG_STRING_EVENT: DWORD = 8;
pub const RIP_EVENT: DWORD = 9;

pub const EXCEPTION_BREAKPOINT: DWORD = 0x80000003;
pub const STATUS_GUARD_PAGE_VIOLATION: DWORD = 0x80000001;
pub const STATUS_SINGLE_STEP: DWORD = 0x80000004;

pub type PVOID = *mut c_void;
pub type ULONG_PTR = usize;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct EXCEPTION_RECORD {
    pub ExceptionCode: DWORD,
    pub ExceptionFlags: DWORD,
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ExceptionAddress: PVOID,
    pub NumberParameters: DWORD,
    pub ExceptionInformation: [ULONG_PTR; 15],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct EXCEPTION_DEBUG_INFO {
    pub ExceptionRecord: EXCEPTION_RECORD,
    pub dwFirstChance: DWORD,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CREATE_THREAD_DEBUG_INFO {
    pub hThread: HANDLE,
    pub lpThreadLocalBase: LPVOID,
    pub lpStartAddress: LPVOID,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CREATE_PROCESS_DEBUG_INFO {
    pub hFile: HANDLE,
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub lpBaseOfImage: LPVOID,
    pub dwDebugInfoFileOffset: DWORD,
    pub nDebugInfoSize: DWORD,
    pub lpThreadLocalBase: LPVOID,
    pub lpStartAddress: LPVOID,
}

#[repr(C)]
pub struct DEBUG_EVENT {
    pub dwDebugEventCode: DWORD,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
    pub u: DEBUG_EVENT_UNION,
}

#[repr(C)]
pub union DEBUG_EVENT_UNION {
    pub Exception: EXCEPTION_DEBUG_INFO,
    pub CreateThread: CREATE_THREAD_DEBUG_INFO,
    pub CreateProcessInfo: CREATE_PROCESS_DEBUG_INFO,
    pub Raw: [u8; 160],
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct CONTEXT {
    pub P1Home: u64,
    pub P2Home: u64,
    pub P3Home: u64,
    pub P4Home: u64,
    pub P5Home: u64,
    pub P6Home: u64,
    
    pub ContextFlags: DWORD,
    pub MxCsr: DWORD,
    
    pub SegCs: WORD,
    pub SegDs: WORD,
    pub SegEs: WORD,
    pub SegFs: WORD,
    pub SegGs: WORD,
    pub SegSs: WORD,
    pub EFlags: DWORD,
    
    pub Dr0: u64,
    pub Dr1: u64,
    pub Dr2: u64,
    pub Dr3: u64,
    pub Dr6: u64,
    pub Dr7: u64,
    
    pub Rax: u64,
    pub Rcx: u64,
    pub Rdx: u64,
    pub Rbx: u64,
    pub Rsp: u64,
    pub Rbp: u64,
    pub Rsi: u64,
    pub Rdi: u64,
    pub R8: u64,
    pub R9: u64,
    pub R10: u64,
    pub R11: u64,
    pub R12: u64,
    pub R13: u64,
    pub R14: u64,
    pub R15: u64,
    
    pub Rip: u64,
    
    pub Header: [u8; 512],
    pub VectorRegister: [M128A; 26],
    pub VectorControl: u64,
    
    pub DebugControl: u64,
    pub LastBranchToRip: u64,
    pub LastBranchFromRip: u64,
    pub LastExceptionToRip: u64,
    pub LastExceptionFromRip: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct M128A {
    pub Low: u64,
    pub High: i64,
}

pub type LPCONTEXT = *mut CONTEXT;
pub const CONTEXT_CONTROL: DWORD = 0x00100001;
pub const CONTEXT_INTEGER: DWORD = 0x00100002;
pub const CONTEXT_FULL: DWORD = CONTEXT_CONTROL | CONTEXT_INTEGER;
pub const THREAD_ALL_ACCESS: DWORD = 0x001F03FF;

extern "system" {
    pub fn WaitForDebugEvent(lpDebugEvent: *mut DEBUG_EVENT, dwMilliseconds: DWORD) -> BOOL;
    pub fn ContinueDebugEvent(dwProcessId: DWORD, dwThreadId: DWORD, dwContinueStatus: DWORD) -> BOOL;
    pub fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    pub fn OpenThread(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwThreadId: DWORD) -> HANDLE;
    pub fn ResumeThread(hThread: HANDLE) -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn ReadProcessMemory(
        hProcess: HANDLE,
        lpBaseAddress: LPCVOID,
        lpBuffer: LPVOID,
        nSize: SIZE_T,
        lpNumberOfBytesRead: *mut SIZE_T,
    ) -> BOOL;
    pub fn WriteProcessMemory(
        hProcess: HANDLE,
        lpBaseAddress: LPVOID,
        lpBuffer: LPCVOID,
        nSize: SIZE_T,
        lpNumberOfBytesWritten: *mut SIZE_T,
    ) -> BOOL;
    pub fn FlushInstructionCache(hProcess: HANDLE, lpBaseAddress: LPCVOID, dwSize: SIZE_T) -> BOOL;
    pub fn GetThreadContext(hThread: HANDLE, lpContext: LPCONTEXT) -> BOOL;
    pub fn SetThreadContext(hThread: HANDLE, lpContext: LPCVOID) -> BOOL;
    pub fn DebugActiveProcess(dwProcessId: DWORD) -> BOOL;
    pub fn DebugActiveProcessStop(dwProcessId: DWORD) -> BOOL;
    pub fn GetModuleHandleA(lpModuleName: *const u8) -> HANDLE;
    pub fn GetProcAddress(hModule: HANDLE, lpProcName: *const u8) -> LPVOID;
}

struct ActiveCall {
    buffer_addr: u64,
    bytes_read_ptr: u64,
    return_addr: u64,
}

pub struct WindowsInterceptor {
    pid: u32,
    h_process: HANDLE,
    h_thread: HANDLE,
    read_file_addr: LPVOID,
    original_read_file_byte: u8,
    time_addr: LPVOID,
    original_time_byte: u8,
    active_read_file_call: Option<ActiveCall>,
    active_time_call: Option<ActiveCall>,
    return_breakpoints: std::collections::HashMap<u64, u8>,
    next_event_id: u64,
}

unsafe impl Send for WindowsInterceptor {}

impl WindowsInterceptor {
    pub fn new() -> Self {
        Self {
            pid: 0,
            h_process: std::ptr::null_mut(),
            h_thread: std::ptr::null_mut(),
            read_file_addr: std::ptr::null_mut(),
            original_read_file_byte: 0,
            time_addr: std::ptr::null_mut(),
            original_time_byte: 0,
            active_read_file_call: None,
            active_time_call: None,
            return_breakpoints: std::collections::HashMap::new(),
            next_event_id: 0,
        }
    }

    fn install_breakpoints(&mut self) -> Result<(), RevError> {
        if !self.read_file_addr.is_null() {
            let mut original = 0u8;
            let mut read = 0;
            unsafe {
                ReadProcessMemory(
                    self.h_process,
                    self.read_file_addr,
                    &mut original as *mut _ as LPVOID,
                    1,
                    &mut read,
                );
            }
            if read == 1 {
                self.original_read_file_byte = original;
                let cc = 0xCCu8;
                let mut written = 0;
                unsafe {
                    WriteProcessMemory(
                        self.h_process,
                        self.read_file_addr,
                        &cc as *const _ as LPCVOID,
                        1,
                        &mut written,
                    );
                    FlushInstructionCache(self.h_process, self.read_file_addr, 1);
                }
            }
        }

        if !self.time_addr.is_null() {
            let mut original = 0u8;
            let mut read = 0;
            unsafe {
                ReadProcessMemory(
                    self.h_process,
                    self.time_addr,
                    &mut original as *mut _ as LPVOID,
                    1,
                    &mut read,
                );
            }
            if read == 1 {
                self.original_time_byte = original;
                let cc = 0xCCu8;
                let mut written = 0;
                unsafe {
                    WriteProcessMemory(
                        self.h_process,
                        self.time_addr,
                        &cc as *const _ as LPCVOID,
                        1,
                        &mut written,
                    );
                    FlushInstructionCache(self.h_process, self.time_addr, 1);
                }
            }
        }
        Ok(())
    }

    fn get_thread_handle(&self, thread_id: DWORD) -> Result<HANDLE, RevError> {
        let h = unsafe { OpenThread(THREAD_ALL_ACCESS, 0, thread_id) };
        if h.is_null() {
            Err(RevError::Io(std::io::Error::last_os_error()))
        } else {
            Ok(h)
        }
    }

    fn handle_read_file_entry(&mut self, thread_id: DWORD) -> Result<(), RevError> {
        let mut context = unsafe { std::mem::zeroed::<CONTEXT>() };
        context.ContextFlags = CONTEXT_FULL;
        
        let h_thread = self.get_thread_handle(thread_id)?;
        if unsafe { GetThreadContext(h_thread, &mut context) } == 0 {
            unsafe { CloseHandle(h_thread); }
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        let lp_buffer = context.Rdx;
        let lp_bytes_read = context.R9;

        let mut return_addr = 0u64;
        let mut read = 0;
        unsafe {
            ReadProcessMemory(
                self.h_process,
                context.Rsp as LPCVOID,
                &mut return_addr as *mut _ as LPVOID,
                8,
                &mut read,
            );
        }

        if read == 8 {
            let mut original_byte = 0u8;
            let mut read_b = 0;
            unsafe {
                ReadProcessMemory(
                    self.h_process,
                    return_addr as LPCVOID,
                    &mut original_byte as *mut _ as LPVOID,
                    1,
                    &mut read_b,
                );
            }
            if read_b == 1 {
                self.return_breakpoints.insert(return_addr, original_byte);
                let cc = 0xCCu8;
                let mut written = 0;
                unsafe {
                    WriteProcessMemory(
                        self.h_process,
                        return_addr as LPVOID,
                        &cc as *const _ as LPCVOID,
                        1,
                        &mut written,
                    );
                    FlushInstructionCache(self.h_process, return_addr as LPCVOID, 1);
                }
            }

            self.active_read_file_call = Some(ActiveCall {
                buffer_addr: lp_buffer,
                bytes_read_ptr: lp_bytes_read,
                return_addr,
            });
        }

        let mut written = 0;
        unsafe {
            WriteProcessMemory(
                self.h_process,
                self.read_file_addr,
                &self.original_read_file_byte as *const _ as LPCVOID,
                1,
                &mut written,
            );
            FlushInstructionCache(self.h_process, self.read_file_addr, 1);
        }
        
        context.Rip = self.read_file_addr as u64;
        context.EFlags |= 0x100;
        unsafe {
            SetThreadContext(h_thread, &context as *const CONTEXT as LPCVOID);
            CloseHandle(h_thread);
        }

        Ok(())
    }

    fn handle_time_entry(&mut self, thread_id: DWORD) -> Result<(), RevError> {
        let mut context = unsafe { std::mem::zeroed::<CONTEXT>() };
        context.ContextFlags = CONTEXT_FULL;
        
        let h_thread = self.get_thread_handle(thread_id)?;
        if unsafe { GetThreadContext(h_thread, &mut context) } == 0 {
            unsafe { CloseHandle(h_thread); }
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        let lp_time = context.Rcx;

        let mut return_addr = 0u64;
        let mut read = 0;
        unsafe {
            ReadProcessMemory(
                self.h_process,
                context.Rsp as LPCVOID,
                &mut return_addr as *mut _ as LPVOID,
                8,
                &mut read,
            );
        }

        if read == 8 {
            let mut original_byte = 0u8;
            let mut read_b = 0;
            unsafe {
                ReadProcessMemory(
                    self.h_process,
                    return_addr as LPCVOID,
                    &mut original_byte as *mut _ as LPVOID,
                    1,
                    &mut read_b,
                );
            }
            if read_b == 1 {
                self.return_breakpoints.insert(return_addr, original_byte);
                let cc = 0xCCu8;
                let mut written = 0;
                unsafe {
                    WriteProcessMemory(
                        self.h_process,
                        return_addr as LPVOID,
                        &cc as *const _ as LPCVOID,
                        1,
                        &mut written,
                    );
                    FlushInstructionCache(self.h_process, return_addr as LPCVOID, 1);
                }
            }

            self.active_time_call = Some(ActiveCall {
                buffer_addr: lp_time,
                bytes_read_ptr: 0,
                return_addr,
            });
        }

        let mut written = 0;
        unsafe {
            WriteProcessMemory(
                self.h_process,
                self.time_addr,
                &self.original_time_byte as *const _ as LPCVOID,
                1,
                &mut written,
            );
            FlushInstructionCache(self.h_process, self.time_addr, 1);
        }
        
        context.Rip = self.time_addr as u64;
        context.EFlags |= 0x100;
        unsafe {
            SetThreadContext(h_thread, &context as *const CONTEXT as LPCVOID);
            CloseHandle(h_thread);
        }

        Ok(())
    }

    fn handle_exit_breakpoint(&mut self, thread_id: DWORD, addr: u64) -> Result<Option<SyscallEvent>, RevError> {
        let mut context = unsafe { std::mem::zeroed::<CONTEXT>() };
        context.ContextFlags = CONTEXT_FULL;
        
        let h_thread = self.get_thread_handle(thread_id)?;
        if unsafe { GetThreadContext(h_thread, &mut context) } == 0 {
            unsafe { CloseHandle(h_thread); }
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }

        let mut event = None;

        if let Some(call) = &self.active_read_file_call {
            if call.return_addr == addr {
                let success = context.Rax != 0;
                let mut bytes_read = 0usize;
                
                if success {
                    let mut bytes_count = 0u32;
                    let mut read = 0;
                    unsafe {
                        ReadProcessMemory(
                            self.h_process,
                            call.bytes_read_ptr as LPCVOID,
                            &mut bytes_count as *mut _ as LPVOID,
                            4,
                            &mut read,
                        );
                    }
                    if read == 4 {
                        bytes_read = bytes_count as usize;
                    }
                }

                let mut buf = vec![0u8; bytes_read];
                if bytes_read > 0 {
                    let mut read_b = 0;
                    unsafe {
                        ReadProcessMemory(
                            self.h_process,
                            call.buffer_addr as LPCVOID,
                            buf.as_mut_ptr() as LPVOID,
                            bytes_read,
                            &mut read_b,
                        );
                    }
                }

                event = Some(SyscallEvent {
                    id: self.next_event_id,
                    timestamp_ns: std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64,
                    syscall: rev_core::types::SyscallKind::FileRead {
                        path: None,
                    },
                    return_bytes: buf,
                    fd: None,
                });
                self.next_event_id += 1;
                self.active_read_file_call = None;
            }
        }
        
        if let Some(call) = &self.active_time_call {
            if call.return_addr == addr {
                let mut filetime_bytes = vec![0u8; 8];
                let mut read = 0;
                unsafe {
                    ReadProcessMemory(
                        self.h_process,
                        call.buffer_addr as LPCVOID,
                        filetime_bytes.as_mut_ptr() as LPVOID,
                        8,
                        &mut read,
                    );
                }
                
                event = Some(SyscallEvent {
                    id: self.next_event_id,
                    timestamp_ns: std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64,
                    syscall: rev_core::types::SyscallKind::TimeRead,
                    return_bytes: filetime_bytes,
                    fd: None,
                });
                self.next_event_id += 1;
                self.active_time_call = None;
            }
        }

        if let Some(&original_byte) = self.return_breakpoints.get(&addr) {
            let mut written = 0;
            unsafe {
                WriteProcessMemory(
                    self.h_process,
                    addr as LPVOID,
                    &original_byte as *const _ as LPCVOID,
                    1,
                    &mut written,
                );
                FlushInstructionCache(self.h_process, addr as LPCVOID, 1);
            }
        }

        context.Rip = addr;
        context.EFlags |= 0x100;
        unsafe {
            SetThreadContext(h_thread, &context as *const CONTEXT as LPCVOID);
            CloseHandle(h_thread);
        }

        Ok(event)
    }

    fn reinstall_breakpoints_after_step(&mut self, thread_id: DWORD) -> Result<(), RevError> {
        let h_thread = self.get_thread_handle(thread_id)?;
        let mut context = unsafe { std::mem::zeroed::<CONTEXT>() };
        context.ContextFlags = CONTEXT_CONTROL;
        
        if unsafe { GetThreadContext(h_thread, &mut context) } != 0 {
            context.EFlags &= !0x100;
            unsafe {
                SetThreadContext(h_thread, &context as *const CONTEXT as LPCVOID);
            }
        }
        unsafe { CloseHandle(h_thread); }

        let cc = 0xCCu8;
        let mut written = 0;
        if !self.read_file_addr.is_null() {
            unsafe {
                WriteProcessMemory(self.h_process, self.read_file_addr, &cc as *const _ as LPCVOID, 1, &mut written);
                FlushInstructionCache(self.h_process, self.read_file_addr, 1);
            }
        }
        if !self.time_addr.is_null() {
            unsafe {
                WriteProcessMemory(self.h_process, self.time_addr, &cc as *const _ as LPCVOID, 1, &mut written);
                FlushInstructionCache(self.h_process, self.time_addr, 1);
            }
        }

        for &addr in self.return_breakpoints.keys() {
            unsafe {
                WriteProcessMemory(self.h_process, addr as LPVOID, &cc as *const _ as LPCVOID, 1, &mut written);
                FlushInstructionCache(self.h_process, addr as LPCVOID, 1);
            }
        }

        Ok(())
    }
}

impl Interceptor for WindowsInterceptor {
    fn attach(&mut self, pid: u32) -> Result<(), RevError> {
        self.pid = pid;
        let res = unsafe { DebugActiveProcess(pid) };
        if res == 0 {
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }
        self.h_process = unsafe { OpenProcess(0x1F0FFF, 0, pid) };
        if self.h_process.is_null() {
            return Err(RevError::Io(std::io::Error::last_os_error()));
        }
        
        let kernel32 = unsafe { GetModuleHandleA(b"kernel32.dll\0".as_ptr()) };
        if !kernel32.is_null() {
            self.read_file_addr = unsafe { GetProcAddress(kernel32, b"ReadFile\0".as_ptr()) };
            self.time_addr = unsafe { GetProcAddress(kernel32, b"GetSystemTimeAsFileTime\0".as_ptr()) };
        }

        self.install_breakpoints()?;
        Ok(())
    }

    fn next_event(&mut self) -> Result<SyscallEvent, RevError> {
        let mut debug_event = unsafe { std::mem::zeroed::<DEBUG_EVENT>() };
        
        loop {
            let res = unsafe { WaitForDebugEvent(&mut debug_event, INFINITE) };
            if res == 0 {
                return Err(RevError::Io(std::io::Error::last_os_error()));
            }

            let mut continue_status = DBG_CONTINUE;
            let mut syscall_event = None;

            match debug_event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    let info = unsafe { debug_event.u.CreateProcessInfo };
                    self.h_thread = info.hThread;
                    unsafe {
                        ResumeThread(info.hThread);
                    }
                    let _ = self.install_breakpoints();
                }
                EXCEPTION_DEBUG_EVENT => {
                    let info = unsafe { debug_event.u.Exception };
                    let record = info.ExceptionRecord;
                    let addr = record.ExceptionAddress;
                    
                    if record.ExceptionCode == EXCEPTION_BREAKPOINT {
                        if addr == self.read_file_addr {
                            self.handle_read_file_entry(debug_event.dwThreadId)?;
                        } else if addr == self.time_addr {
                            self.handle_time_entry(debug_event.dwThreadId)?;
                        } else if self.return_breakpoints.contains_key(&(addr as u64)) {
                            if let Some(event) = self.handle_exit_breakpoint(debug_event.dwThreadId, addr as u64)? {
                                syscall_event = Some(event);
                            }
                        } else {
                            continue_status = DBG_EXCEPTION_NOT_HANDLED;
                        }
                    } else if record.ExceptionCode == STATUS_SINGLE_STEP {
                        self.reinstall_breakpoints_after_step(debug_event.dwThreadId)?;
                    } else {
                        continue_status = DBG_EXCEPTION_NOT_HANDLED;
                    }
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    return Err(RevError::ReplayFailed {
                        step: self.next_event_id,
                        reason: "Target process exited".to_string(),
                    });
                }
                _ => {}
            }

            unsafe {
                ContinueDebugEvent(
                    debug_event.dwProcessId,
                    debug_event.dwThreadId,
                    continue_status,
                );
            }

            if let Some(event) = syscall_event {
                return Ok(event);
            }
        }
    }

    fn detach(&mut self) -> Result<(), RevError> {
        let mut written = 0;
        if !self.read_file_addr.is_null() && self.original_read_file_byte != 0 {
            unsafe {
                WriteProcessMemory(
                    self.h_process,
                    self.read_file_addr,
                    &self.original_read_file_byte as *const _ as LPCVOID,
                    1,
                    &mut written,
                );
            }
        }
        if !self.time_addr.is_null() && self.original_time_byte != 0 {
            unsafe {
                WriteProcessMemory(
                    self.h_process,
                    self.time_addr,
                    &self.original_time_byte as *const _ as LPCVOID,
                    1,
                    &mut written,
                );
            }
        }

        for (&addr, &original_byte) in &self.return_breakpoints {
            unsafe {
                WriteProcessMemory(
                    self.h_process,
                    addr as LPVOID,
                    &original_byte as *const _ as LPCVOID,
                    1,
                    &mut written,
                );
            }
        }

        if self.pid != 0 {
            unsafe {
                DebugActiveProcessStop(self.pid);
            }
        }

        if !self.h_process.is_null() {
            unsafe {
                CloseHandle(self.h_process);
            }
            self.h_process = std::ptr::null_mut();
        }

        Ok(())
    }
}
