use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;

#[cfg(not(target_os = "windows"))]
use {
    aes::Aes128,
    aes::Aes256,
    cbc::cipher::{BlockModeDecrypt, KeyIvInit},
    pbkdf2::pbkdf2_hmac,
    sha1::Sha1,
};

#[cfg(target_os = "windows")]
use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

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
fn decrypt_dpapi(data: &[u8]) -> Result<String> {
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::System::Memory::LocalFree;

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

        let bytes = std::slice::from_raw_parts(
            output.pbData,
            output.cbData as usize,
        ).to_vec();

        LocalFree(HLOCAL(output.pbData as _));

        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn get_keyring_password() -> Result<Vec<u8>> {
    let candidates = [
        ("application", "chrome"),
        ("application", "chromium"),
        ("application", "Steam"),
    ];

    for (attr_key, attr_val) in candidates {
        let output = std::process::Command::new("secret-tool")
            .args(["lookup", attr_key, attr_val])
            .output();

        if let Ok(out) = output {
            if out.status.success() && !out.stdout.is_empty() {
                let password = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .to_string();
                return Ok(password.into_bytes());
            }
        }
    }

    anyhow::bail!("No keyring password found via secret-tool")
}

#[cfg(not(target_os = "windows"))]
fn decrypt_cookie_linux(encrypted: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted value too short");
    }

    let prefix = &encrypted[..3];
    if prefix != b"v10" && prefix != b"v11" {
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    let ciphertext = &encrypted[3..];
    let iv = [b' '; 16];
    let password = get_keyring_password().unwrap_or_else(|_| b"peanuts".to_vec());

    // Try AES-128 first (v10), then AES-256 (v11)
    let candidates_128: &[&[u8]] = &[password.as_slice(), b"peanuts", b""];
    for pw in candidates_128 {
        let mut key = [0u8; 16];
        pbkdf2_hmac::<Sha1>(pw, b"saltysalt", 1, &mut key);

        type Aes128CbcDec = cbc::Decryptor<Aes128>;
        let decryptor = Aes128CbcDec::new(&key.into(), &iv.into());
        let mut buf = ciphertext.to_vec();

        if let Ok(decrypted) = decryptor
            .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
        {
            return Ok(String::from_utf8_lossy(decrypted).to_string());
        }
    }

    let candidates_256: &[&[u8]] = &[password.as_slice(), b"peanuts", b""];
    for pw in candidates_256 {
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha1>(pw, b"saltysalt", 1, &mut key);

        type Aes256CbcDec = cbc::Decryptor<Aes256>;
        let decryptor = Aes256CbcDec::new(&key.into(), &iv.into());
        let mut buf = ciphertext.to_vec();

        if let Ok(decrypted) = decryptor
            .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
        {
            return Ok(String::from_utf8_lossy(decrypted).to_string());
        }
    }

    anyhow::bail!("All decryption attempts failed")
}

fn decrypt_cookie(encrypted: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted value too short");
    }

    #[cfg(target_os = "windows")]
    {
        let prefix = &encrypted[..3];
        // On Windows, v10 = DPAPI blob; no prefix = plaintext
        if prefix == b"v10" || prefix == b"v11" {
            return decrypt_dpapi(&encrypted[3..]);
        }
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        return decrypt_cookie_linux(encrypted);
    }
}

pub fn get_steam_cookies() -> Result<SteamCookies> {
    let db_path = get_cookie_db_path().context("Failed to get cookie DB path")?;
    eprintln!("Cookie DB path: {}", db_path.display());

    let tmp_path = std::env::temp_dir().join("swu_cookies_tmp.db");
    eprintln!("Temp path: {}", tmp_path.display());

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
        eprintln!("Row: name={} value_len={} encrypted_len={}", name, value.len(), encrypted.len());

        if !encrypted.is_empty() {
            eprintln!("  encrypted prefix: {:?}", &encrypted[..encrypted.len().min(4)]);
        }

        let resolved = if !value.is_empty() {
            eprintln!("  using plaintext value");
            value
        } else if !encrypted.is_empty() {
            eprintln!("  attempting decryption...");
            match decrypt_cookie(&encrypted) {
                Ok(v) => {
                    eprintln!("  decrypted ok, len={}", v.len());
                    v
                }
                Err(e) => {
                    eprintln!("  decryption error: {}", e);
                    String::new()
                }
            }
        } else {
            eprintln!("  no value and no encrypted_value");
            String::new()
        };

        match name.as_str() {
            "sessionid" => session_id = Some(resolved),
            "steamLoginSecure" => login_secure = Some(resolved),
            _ => {}
        }
    }

    let _ = std::fs::remove_file(&tmp_path);

    eprintln!("session_id present: {}", session_id.is_some());
    eprintln!("login_secure present: {}", login_secure.is_some());

    Ok(SteamCookies {
        session_id: session_id.context("sessionid cookie not found — are you logged into Steam?")?,
        login_secure: login_secure.context("steamLoginSecure cookie not found — are you logged into Steam?")?,
    })
}