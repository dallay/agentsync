#![cfg(windows)]

use std::ffi::c_void;
use std::fs;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::Mutex;

use agentsync::gitignore::{cleanup_gitignore, update_gitignore};
use tempfile::TempDir;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, HLOCAL, LocalFree, SetLastError,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSidToSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, AdjustTokenPrivileges, DACL_SECURITY_INFORMATION, GetLengthSid,
    GetSecurityDescriptorControl, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED,
    SE_PRIVILEGE_ENABLED, SE_RESTORE_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

static WINDOWS_SECURITY_TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
struct SecuritySnapshot {
    owner_sid: Vec<u8>,
    dacl: Option<Vec<u8>>,
    dacl_protected: bool,
}

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: The allocation is returned by a Windows security API
            // documented to allocate memory released by LocalFree.
            unsafe {
                LocalFree(self.0 as HLOCAL);
            }
        }
    }
}

struct TokenHandle(HANDLE);

impl Drop for TokenHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: OpenProcessToken created this owned handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

struct RestorePrivilegeGuard {
    token: TokenHandle,
    previous: TOKEN_PRIVILEGES,
}

impl Drop for RestorePrivilegeGuard {
    fn drop(&mut self) {
        // SAFETY: The token remains open and `previous` contains the state
        // returned by AdjustTokenPrivileges when the privilege was enabled.
        unsafe {
            AdjustTokenPrivileges(self.token.0, 0, &self.previous, 0, null_mut(), null_mut());
        }
    }
}

fn set_restore_privilege_enabled(enabled: bool) -> io::Result<RestorePrivilegeGuard> {
    let mut raw_token: HANDLE = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle and OpenProcessToken
    // writes an owned token handle to `raw_token` on success.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut raw_token,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let token = TokenHandle(raw_token);

    let mut privilege_luid = windows_sys::Win32::Foundation::LUID::default();
    // SAFETY: SE_RESTORE_NAME is a static NUL-terminated Windows string and
    // `privilege_luid` is a valid output location.
    if unsafe { LookupPrivilegeValueW(null_mut(), SE_RESTORE_NAME, &mut privilege_luid) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let requested = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: privilege_luid,
            Attributes: if enabled { SE_PRIVILEGE_ENABLED } else { 0 },
        }],
    };
    let mut previous = TOKEN_PRIVILEGES::default();
    let mut returned_length = 0;
    // A successful AdjustTokenPrivileges call may still leave the requested
    // privilege unassigned; clear and inspect last error as required by Win32.
    unsafe {
        SetLastError(0);
    }
    // SAFETY: Both privilege structures and the returned-length output remain
    // valid for the duration of the call; `token` holds the required access.
    if unsafe {
        AdjustTokenPrivileges(
            token.0,
            0,
            &requested,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            &mut previous,
            &mut returned_length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_NOT_ALL_ASSIGNED {
        return Err(io::Error::from_raw_os_error(last_error as i32));
    }

    Ok(RestorePrivilegeGuard { token, previous })
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn security_snapshot(path: &Path) -> io::Result<SecuritySnapshot> {
    let wide_path = wide_path(path);
    let mut owner: PSID = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();

    // SAFETY: The output pointers are valid and the NUL-terminated path stays
    // alive for the duration of the call. The returned descriptor is freed
    // after copying the owner SID and DACL bytes below.
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if descriptor.is_null() || owner.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned an empty security descriptor or owner SID",
        ));
    }
    let _descriptor = LocalAllocation(descriptor.cast());

    // SAFETY: `owner` points into the live descriptor returned above;
    // GetLengthSid validates and reports the SID's byte length.
    let owner_length = unsafe { GetLengthSid(owner) } as usize;
    if owner_length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned an invalid owner SID",
        ));
    }
    let owner_sid =
        unsafe { std::slice::from_raw_parts(owner.cast::<u8>(), owner_length) }.to_vec();

    let dacl_bytes = if dacl.is_null() {
        None
    } else {
        // SAFETY: `dacl` points into the live security descriptor returned by
        // GetNamedSecurityInfoW; AclSize bounds the ACL byte range.
        let length = unsafe { (*dacl).AclSize as usize };
        Some(unsafe { std::slice::from_raw_parts(dacl.cast::<u8>(), length) }.to_vec())
    };
    let mut control = 0;
    let mut revision = 0;
    // SAFETY: The descriptor remains live until this function returns.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(SecuritySnapshot {
        owner_sid,
        dacl: dacl_bytes,
        dacl_protected: control & SE_DACL_PROTECTED != 0,
    })
}

fn set_owner_sid(path: &Path, sid: &str) -> io::Result<()> {
    let wide_path = wide_path(path);
    let wide_sid = sid
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut owner: PSID = null_mut();

    // SAFETY: The SID string is NUL-terminated and the output pointer is valid.
    if unsafe { ConvertStringSidToSidW(wide_sid.as_ptr(), &mut owner) } == 0 || owner.is_null() {
        return Err(io::Error::last_os_error());
    }
    let _owner = LocalAllocation(owner.cast());

    // SAFETY: The SID allocation and NUL-terminated path remain alive for the
    // call. Only OWNER_SECURITY_INFORMATION is requested; the DACL is untouched.
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            owner,
            null_mut(),
            null_mut(),
            null_mut(),
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

#[test]
fn update_and_cleanup_preserve_gitignore_owner_sid_and_dacl() {
    const BUILTIN_USERS_SID: &str = "S-1-5-32-545";

    let _serial = WINDOWS_SECURITY_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore_privilege = set_restore_privilege_enabled(true)
        .expect("test runner must have SeRestorePrivilege to construct a foreign-owner fixture");
    let temp = TempDir::new().unwrap();
    let gitignore = temp.path().join(".gitignore");
    fs::write(&gitignore, "existing-rule\n").unwrap();

    let created_owner = security_snapshot(&gitignore).unwrap().owner_sid;
    set_owner_sid(&gitignore, BUILTIN_USERS_SID)
        .expect("test runner must be able to set the fixture owner SID");
    let expected = security_snapshot(&gitignore).unwrap();
    assert_ne!(
        expected.owner_sid, created_owner,
        "the fixture must have an owner SID different from the creator"
    );
    update_gitignore(
        temp.path(),
        "AgentSync",
        &["generated.md".to_string()],
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        security_snapshot(&gitignore).unwrap(),
        expected,
        "atomic update must preserve the original owner SID and DACL"
    );

    cleanup_gitignore(temp.path(), "AgentSync", false, false).unwrap();
    assert_eq!(
        security_snapshot(&gitignore).unwrap(),
        expected,
        "atomic cleanup must preserve the original owner SID and DACL"
    );
}

#[test]
fn update_fails_closed_and_keeps_original_when_owner_sid_cannot_be_restored() {
    const BUILTIN_USERS_SID: &str = "S-1-5-32-545";

    let _serial = WINDOWS_SECURITY_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore_privilege = set_restore_privilege_enabled(true)
        .expect("test runner must have SeRestorePrivilege to construct a foreign-owner fixture");
    let temp = TempDir::new().unwrap();
    let gitignore = temp.path().join(".gitignore");
    let original_contents = "existing-rule\n";
    fs::write(&gitignore, original_contents).unwrap();
    set_owner_sid(&gitignore, BUILTIN_USERS_SID).unwrap();
    let expected_security = security_snapshot(&gitignore).unwrap();

    // The original belongs to BUILTIN\Users, but the replacement runs without
    // SeRestorePrivilege. It must fail before atomic persist rather than leave
    // a staged file with a different owner SID.
    let _restore_disabled = set_restore_privilege_enabled(false)
        .expect("test runner must be able to disable SeRestorePrivilege temporarily");
    let result = update_gitignore(
        temp.path(),
        "AgentSync",
        &["generated.md".to_string()],
        false,
        false,
    );

    assert!(
        result.is_err(),
        "update must fail closed when the original owner SID cannot be restored"
    );
    assert_eq!(fs::read_to_string(&gitignore).unwrap(), original_contents);
    assert_eq!(security_snapshot(&gitignore).unwrap(), expected_security);
}
