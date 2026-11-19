"""Windows process primitives for the isolated bench instance (#2905).

Windows has no ``/proc``, so the bench manager identifies instance processes
with two things it controls itself:

* a launch manifest recording ``pid`` + process creation time (a PID-reuse
  guard), and
* a **named job object** embedding a random per-launch token; every process
  the instance spawns joins that job, and ``TerminateJobObject`` stops the
  whole tree, detached sidecars included.

Processes are never selected by the image name ``libragent``. The job name
contains the per-launch token, so a stop can only ever reach the process tree
this manifest launched. Call these functions only on Windows (``sys.platform
== "win32"``); the module imports cleanly elsewhere so tests can stub it.
"""

from __future__ import annotations

import ctypes
import sys
from ctypes import wintypes

_JOB_OBJECT_TERMINATE = 0x0010
_PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
_STILL_ACTIVE = 259


class WinProcessError(RuntimeError):
    """A Win32 call needed by the bench manager failed."""


def _kernel32() -> ctypes.WinDLL:  # pragma: no cover - Windows only
    if sys.platform != "win32":
        raise WinProcessError("win_process helpers are Windows-only")
    return ctypes.WinDLL("kernel32", use_last_error=True)


def _check(handle: int, what: str) -> int:
    if not handle:
        raise WinProcessError(f"{what} failed: {ctypes.get_last_error()}")
    return handle


def pid_alive(pid: int) -> bool:
    """Whether ``pid`` still refers to a running process."""
    kernel32 = _kernel32()
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    handle = kernel32.OpenProcess(_PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return False
    try:
        kernel32.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
        exit_code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
            return False
        return exit_code.value == _STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def process_creation_time(pid: int) -> int | None:
    """Kernel-file-time creation stamp of ``pid``, or ``None`` if unavailable.

    Persisted in the launch manifest and re-checked before every signal: a
    recycled PID stamps differently, so a stale ``instance.pid`` can never
    point at an unrelated process.
    """
    kernel32 = _kernel32()
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    handle = kernel32.OpenProcess(_PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return None
    try:
        kernel32.GetProcessTimes.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
        ]
        created = wintypes.FILETIME()
        if not kernel32.GetProcessTimes(handle, ctypes.byref(created), None, None, None):
            return None
        return (created.dwHighDateTime << 32) | created.dwLowDateTime
    finally:
        kernel32.CloseHandle(handle)


def create_job(name: str) -> int:
    """Create (or open) the named job object and return its handle."""
    kernel32 = _kernel32()
    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    return _check(kernel32.CreateJobObjectW(None, name), "CreateJobObjectW")


def open_job(name: str) -> int | None:
    """Open an existing named job for termination, or ``None`` when absent."""
    kernel32 = _kernel32()
    kernel32.OpenJobObjectW.restype = wintypes.HANDLE
    kernel32.OpenJobObjectW.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.LPCWSTR]
    handle = kernel32.OpenJobObjectW(_JOB_OBJECT_TERMINATE, False, name)
    return handle or None


def assign_process(job: int, process_handle: int) -> None:
    """Put ``process_handle`` into ``job``; processes it spawns later join too."""
    kernel32 = _kernel32()
    kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    if not kernel32.AssignProcessToJobObject(job, process_handle):
        raise WinProcessError(f"AssignProcessToJobObject failed: {ctypes.get_last_error()}")


def terminate_job(job: int) -> None:
    """Kill every process in the job, immediately."""
    kernel32 = _kernel32()
    kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
    if not kernel32.TerminateJobObject(job, 0):
        raise WinProcessError(f"TerminateJobObject failed: {ctypes.get_last_error()}")


def close_handle(handle: int) -> None:
    _kernel32().CloseHandle(handle)


def terminate_job_by_name(name: str) -> bool:
    """Terminate the named job if it still exists; ``False`` when it does not."""
    job = open_job(name)
    if job is None:
        return False
    try:
        terminate_job(job)
    finally:
        close_handle(job)
    return True
