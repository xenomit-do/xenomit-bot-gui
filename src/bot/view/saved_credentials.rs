//! Encrypted storage for account credentials used by the sign-in page.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SavedCredential {
    pub(super) username: String,
    pub(super) password: String,
}

pub(super) struct SavedCredentialStore;

impl SavedCredentialStore {
    pub(super) fn load(path: &Path) -> Result<Vec<SavedCredential>, String> {
        let encrypted = match fs::read(path) {
            Ok(encrypted) => encrypted,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("Could not read saved credentials: {error}")),
        };

        let plaintext = protect::unprotect(&encrypted)?;
        serde_json::from_slice(&plaintext)
            .map_err(|error| format!("Could not read saved credentials: {error}"))
    }

    pub(super) fn save(path: &Path, credentials: &[SavedCredential]) -> Result<(), String> {
        let plaintext = serde_json::to_vec(credentials)
            .map_err(|error| format!("Could not encode saved credentials: {error}"))?;
        let encrypted = protect::protect(&plaintext)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create saved credentials folder: {error}"))?;
        }
        fs::write(path, encrypted).map_err(|error| format!("Could not save credentials: {error}"))
    }
}

#[cfg(windows)]
mod protect {
    use std::ffi::c_void;
    use std::slice;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    pub(super) fn protect(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        transform(plaintext, true)
    }

    pub(super) fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        transform(ciphertext, false)
    }

    fn transform(input: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: input.len() as u32,
            pbData: input.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB::default();

        // DPAPI binds the file to the current Windows user profile.
        let succeeded = unsafe {
            if encrypt {
                CryptProtectData(
                    &input_blob,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            } else {
                CryptUnprotectData(
                    &input_blob,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            }
        };
        if succeeded == 0 {
            return Err(format!(
                "Could not {} saved credentials: {}",
                if encrypt { "protect" } else { "unlock" },
                std::io::Error::last_os_error()
            ));
        }

        let output = unsafe {
            slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
        };
        unsafe {
            LocalFree(output_blob.pbData as *mut c_void);
        }
        Ok(output)
    }
}

#[cfg(not(windows))]
mod protect {
    pub(super) fn protect(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("Saved credentials are only supported on Windows.".into())
    }

    pub(super) fn unprotect(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("Saved credentials are only supported on Windows.".into())
    }
}
