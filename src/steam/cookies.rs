use aes::Aes256;
use anyhow::{Context, Result};
use cbc::cipher::{BlockModeDecrypt, KeyIvInit};
use pbkdf2::pbkdf2_hmac;
use rusqlite::Connection;
use sha1::Sha1;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SteamCookies {
    pub session_id: String,
    pub login_secure: String,
}

fn get_cookie_db_path() -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    let path = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".local/share/Steam/config/htmlcache/Default/Cookies");

    #[cfg(target_os = "windows")]
    let path = dirs::data_local_dir()
        .context("Could not determine local data directory")?
        .join("Steam/htmlcache/Default/Cookies");

    if !path.exists() {
        anyhow::bail!("Steam cookie database not found at {}", path.display());
    }

    Ok(path)
}

#[cfg(target_os = "windows")]
fn decrypt_cookie(encrypted: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted value too short");
    }

    let prefix = &encrypted[..3];
    if prefix != b"v10" {
        // No prefix = already plaintext
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    let ciphertext = &encrypted[3..];
    decrypt_dpapi(ciphertext)
}

#[cfg(target_os = "windows")]
fn decrypt_dpapi(data: &[u8]) -> Result<String> {
    use windows::Win32::Security::Cryptography::CryptUnprotectData;
    use windows::Win32::System::Memory::LocalFree;
    use windows::core::HLOCAL;
    use windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &mut input,
            None,
            None,
            None,
            None,
            0,
            &mut output,
        ).ok().context("CryptUnprotectData failed")?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(HLOCAL(output.pbData as _));
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

fn decrypt_cookie(encrypted: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted value too short");
    }

    let prefix = &encrypted[..3];
    if prefix != b"v10" && prefix != b"v11" {
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let ciphertext = &encrypted[3..];
        return decrypt_dpapi(ciphertext);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let ciphertext = &encrypted[3..];
        let iv = [b' '; 16];

        // v10 = AES-128 (16-byte key), v11 = AES-256 (32-byte key)
        // but many CEF builds just always use 16-byte keys
        let candidates: &[(&[u8], u32)] = &[
            (b"peanuts", 1),
            (b"", 1),
        ];

        // Try AES-128 first (most common for CEF v10)
        for (pw, iterations) in candidates {
            let mut key16 = [0u8; 16];
            pbkdf2_hmac::<Sha1>(pw, b"saltysalt", *iterations, &mut key16);

            use aes::Aes128;
            type Aes128CbcDec = cbc::Decryptor<Aes128>;
            let decryptor = Aes128CbcDec::new(&key16.into(), &iv.into());
            let mut buf = ciphertext.to_vec();

            if let Ok(decrypted) = decryptor
                .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
            {
                return Ok(String::from_utf8_lossy(decrypted).to_string());
            }
        }

        // Fall back to AES-256
        for (pw, iterations) in candidates {
            let mut key32 = [0u8; 32];
            pbkdf2_hmac::<Sha1>(pw, b"saltysalt", *iterations, &mut key32);

            type Aes256CbcDec = cbc::Decryptor<Aes256>;
            let decryptor = Aes256CbcDec::new(&key32.into(), &iv.into());
            let mut buf = ciphertext.to_vec();

            if let Ok(decrypted) = decryptor
                .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
            {
                return Ok(String::from_utf8_lossy(decrypted).to_string());
            }
        }

        anyhow::bail!("All decryption attempts failed")
    }
}

pub fn get_steam_cookies() -> Result<SteamCookies> {
    let db_path = get_cookie_db_path()?;

    let tmp_path = std::env::temp_dir().join("swu_cookies_tmp.db");
    std::fs::copy(&db_path, &tmp_path)
        .context("Could not copy Steam cookie database")?;

    let conn = Connection::open_with_flags(
        &tmp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;

    let mut session_id = None;
    let mut login_secure = None;

    let mut stmt = conn.prepare(
        "SELECT name, value, encrypted_value FROM cookies
         WHERE host_key LIKE '%steamcommunity.com'
         AND name IN ('sessionid', 'steamLoginSecure')"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;

    for row in rows {
        let (name, value, encrypted) = row?;

        let resolved = if !value.is_empty() {
            value
        } else if !encrypted.is_empty() {
            match decrypt_cookie(&encrypted) {
                Ok(v) => {
                    v
                }
                Err(_e) => {
                    String::new()
                }
            }
        } else {
            String::new()
        };

        match name.as_str() {
            "sessionid" => session_id = Some(resolved),
            "steamLoginSecure" => login_secure = Some(resolved),
            _ => {}
        }
    }

    let _ = std::fs::remove_file(&tmp_path);

    Ok(SteamCookies {
        session_id: session_id.context("sessionid cookie not found — are you logged into Steam?")?,
        login_secure: login_secure.context("steamLoginSecure cookie not found — are you logged into Steam?")?,
    })
}