use std::{
    ffi::{CString, OsStr, OsString},
    num::NonZeroIsize,
    os::windows::ffi::{OsStrExt, OsStringExt},
};
use windows::{
    core::{Error, GUID, PCSTR, PCWSTR, PWSTR},
    Win32::{
        Foundation::{HANDLE, HINSTANCE},
        Security::WinTrust::{
            WinVerifyTrustEx, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_DATA_UICONTEXT,
            WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_CHAIN, WTD_REVOKE_NONE,
            WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
        },
        System::{
            LibraryLoader::{
                FreeLibrary, GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32,
            },
            SystemInformation::GetSystemDirectoryW,
        },
    },
};

/// RAII wrapper for a Windows library HANDLE.
// NOTE: This is a `Send` since a `HANDLE` is `Send`
#[repr(transparent)]
pub(crate) struct WindowsLibrary(NonZeroIsize);

impl Drop for WindowsLibrary {
    fn drop(&mut self) {
        unsafe {
            // Deliberately ignoring failure
            let _ignored_result = FreeLibrary(self.as_inner());
        }
    }
}

impl WindowsLibrary {
    /// Open a .dll from C:\Windows\System32 without verification if it's signed
    pub(crate) fn load(lib_name: &str) -> windows::core::Result<Self> {
        let lib_name = CString::new(lib_name).unwrap();

        let lib = unsafe {
            LoadLibraryExA(
                PCSTR(lib_name.as_ptr() as *const u8),
                None,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )?
        };
        // SAFETY: `LoadLibraryExA` returns a non-null pointer on success
        let nonzero = unsafe { NonZeroIsize::new_unchecked(lib.0) };
        Ok(WindowsLibrary(nonzero))
    }

    /// Cast `Library` to HINSTANCE for FFI.
    fn as_inner(&self) -> HINSTANCE {
        HINSTANCE(self.0.get())
    }

    /// Extracts the function pointer from the library. The returned function pointer is bound to
    /// the lifetime `&self`.
    pub(crate) unsafe fn fn_ptr(
        &self,
        fn_name: &str,
    ) -> windows::core::Result<unsafe extern "system" fn() -> isize> {
        let fn_name = CString::new(fn_name).unwrap();
        match GetProcAddress(self.as_inner(), PCSTR(fn_name.as_ptr() as *const u8)) {
            Some(ptr) => Ok(ptr),
            None => Err(windows::core::Error::from_win32()),
        }
    }

    /// Checks if the library is signed. This is different from passing the
    /// `LOAD_LIBRARY_REQUIRE_SIGNED_TARGET` flag to `LoadLibraryExA`.
    // Translated into Rust from:
    // https://docs.microsoft.com/en-us/windows/win32/seccrypto/example-c-program--verifying-the-signature-of-a-pe-file
    pub(crate) fn is_library_signed(filename: &str) -> bool {
        let mut path = get_system32_dir();
        path.push('\\');
        path.push_str(filename);

        let mut wintrust_action_generic_verify_v2 =
            GUID::from_u128(0x00AAC56B_CD44_11d0_8CC2_00C04FC295EE);

        let mut filename: Vec<u16> = OsStr::new(&path).encode_wide().collect();
        filename.push(0);

        let mut file_data = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(filename.as_mut_ptr()),
            hFile: HANDLE(0),
            pgKnownSubject: std::ptr::null_mut(),
        };

        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            pPolicyCallbackData: std::ptr::null_mut(),
            pSIPClientData: std::ptr::null_mut(),
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: WINTRUST_DATA_0 {
                pFile: &mut file_data,
            },
            dwStateAction: WTD_STATEACTION_VERIFY,
            hWVTStateData: HANDLE(0),
            pwszURLReference: PWSTR(std::ptr::null_mut()),
            dwProvFlags: WTD_REVOCATION_CHECK_CHAIN,
            dwUIContext: WINTRUST_DATA_UICONTEXT(0),
            pSignatureSettings: std::ptr::null_mut(),
        };

        let verified = unsafe {
            let s = WinVerifyTrustEx(
                None,
                &mut wintrust_action_generic_verify_v2,
                &mut trust_data,
            );
            s == 0
        };

        trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrustEx(
                None,
                &mut wintrust_action_generic_verify_v2,
                &mut trust_data,
            );
        };
        verified
    }
}

/// Directory to look for .dll's in C:\Windows\System32. This is assumed to be more secure
/// since write access to this directory requires elevated privilege.
fn get_system32_dir() -> String {
    fn inner_fn() -> windows::core::Result<Vec<u16>> {
        let mut expected_length: usize = 19;
        // Add 1 to account for the terminating null character
        let mut buf = vec![0; expected_length + 1];

        loop {
            let size = unsafe { GetSystemDirectoryW(&mut buf) } as usize;
            match Ord::cmp(&(size), &expected_length) {
                // The buffer is too large or there is an error
                std::cmp::Ordering::Less => {
                    if size != 0 {
                        buf.resize(size + 1, 0);
                        break Ok(buf);
                    } else {
                        break Err(Error::from_win32());
                    }
                }
                // The function returns the number of written `u16`s not including the null so
                // expected_length == buf.len() - 1
                std::cmp::Ordering::Equal => break Ok(buf),
                // Confusingly, if the buffer is too small, the function returns the size required
                // _including_ the terminating null character
                std::cmp::Ordering::Greater => {
                    buf.resize(size, 0);
                    expected_length = size - 1;
                }
            }
        }
    }
    let wide = inner_fn().expect("`GetSystemDirectoryW` returned an error");
    // Prevent `OsString` from including the terminating null
    OsString::from_wide(&wide[..(wide.len() - 1)])
        .into_string()
        .expect("Cannot convert the result of `GetSystemDirectoryW` to a `String`")
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_loading() {
        assert!(super::WindowsLibrary::is_library_signed("nvEncodeAPI64.dll") == true);
        super::WindowsLibrary::load("nvEncodeAPI64.dll").unwrap();
    }
}
