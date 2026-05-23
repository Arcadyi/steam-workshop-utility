use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

#[cfg(not(target_os = "windows"))]
use {
    aes::Aes128,
    aes::Aes256,
    cbc::cipher::{BlockModeDecrypt, KeyIvInit},
    pbkdf2::pbkdf2_hmac,
    sha1::Sha1,
};

#[derive(Debug, Clone)]
pub struct SteamCookies {
    pub session_id: String,
    pub login_secure: String,
}

// ---------------------------------------------------------------------------
// Cookie DB path resolution
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn get_cookie_db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .context("Could not determine local data directory")?;

    let candidates = [
        base.join("Steam\\htmlcache\\Default\\Network\\Cookies"),
        base.join("Steam\\htmlcache\\Default\\Cookies"),
        base.join("Steam\\htmlcache\\Cookies"),
        base.join("Steam\\config\\htmlcache\\Default\\Network\\Cookies"),
        base.join("Steam\\config\\htmlcache\\Default\\Cookies"),
        base.join("Steam\\config\\htmlcache\\Cookies"),
        PathBuf::from("C:\\Program Files (x86)\\Steam\\htmlcache\\Default\\Network\\Cookies"),
        PathBuf::from("C:\\Program Files (x86)\\Steam\\htmlcache\\Default\\Cookies"),
        PathBuf::from("C:\\Program Files (x86)\\Steam\\htmlcache\\Cookies"),
        PathBuf::from("C:\\Program Files\\Steam\\htmlcache\\Default\\Network\\Cookies"),
        PathBuf::from("C:\\Program Files\\Steam\\htmlcache\\Default\\Cookies"),
    ];

    candidates
        .into_iter()
        .find(|p| p.exists())
        .context("Steam cookie database not found — is Steam installed and have you logged in?")
}

#[cfg(target_os = "linux")]
fn get_cookie_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let base = home.join(".local/share/Steam/config/htmlcache/Default");

    let candidates = [
        base.join("Network/Cookies"),
        base.join("Cookies"),
    ];

    candidates
        .into_iter()
        .find(|p| p.exists())
        .context("Steam cookie database not found — is Steam installed and have you logged in?")
}

// ---------------------------------------------------------------------------
// Windows: DPAPI + AES-GCM decryption
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn get_aes_key() -> Result<Vec<u8>> {
    use base64::Engine;

    let base = dirs::data_local_dir().context("No local data dir")?;
    let local_state_path = base.join("Steam\\htmlcache\\Local State");

    let contents = std::fs::read_to_string(&local_state_path)
        .context("Could not read Steam Local State file")?;

    let json: serde_json::Value = serde_json::from_str(&contents)
        .context("Could not parse Local State JSON")?;

    let encrypted_key_b64 = json["os_crypt"]["encrypted_key"]
        .as_str()
        .context("No os_crypt.encrypted_key in Local State")?;

    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .context("Could not base64-decode encrypted_key")?;

    if encrypted_key.len() < 5 || &encrypted_key[..5] != b"DPAPI" {
        anyhow::bail!("encrypted_key does not start with expected DPAPI prefix");
    }

    decrypt_dpapi(&encrypted_key[5..]).context("DPAPI decryption of AES key failed")
}

#[cfg(target_os = "windows")]
fn decrypt_dpapi(data: &[u8]) -> Result<Vec<u8>> {
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(&mut input, None, None, None, None, 0, &mut output)
            .ok()
            .context("CryptUnprotectData failed")?;

        let bytes =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();

        windows::Win32::Foundation::LocalFree(Some(HLOCAL(output.pbData as _)));

        Ok(bytes)
    }
}

#[cfg(target_os = "windows")]
fn decrypt_cookie_windows(encrypted: &[u8], aes_key: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted cookie value too short");
    }

    // Legacy cookies encrypted with DPAPI directly (no v10/v11 prefix)
    if !encrypted.starts_with(b"v10") && !encrypted.starts_with(b"v11") {
        let bytes = decrypt_dpapi(encrypted).context("Legacy DPAPI cookie decrypt failed")?;
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    // Modern cookies: AES-256-GCM with 12-byte nonce
    let rest = &encrypted[3..];
    if rest.len() < 12 + 16 {
        anyhow::bail!("Encrypted cookie too short for AES-GCM (need at least 28 bytes after prefix)");
    }

    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};

    let key: &aes_gcm::aead::Key<Aes256Gcm> = aes_key
        .try_into()
        .map_err(|_| anyhow::anyhow!("AES key must be 32 bytes, got {}", aes_key.len()))?;
    let cipher = Aes256Gcm::new(key);

    let nonce: &aes_gcm::Nonce<_> = (&rest[..12])
        .try_into()
        .map_err(|_| anyhow::anyhow!("Nonce must be 12 bytes"))?;

    let plaintext = cipher
        .decrypt(nonce, &rest[12..])
        .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed: {:?}", e))?;

    Ok(String::from_utf8_lossy(&plaintext).to_string())
}

// ---------------------------------------------------------------------------
// Linux: AES-CBC decryption via keyring
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn get_keyring_password() -> Result<Vec<u8>> {
    let candidates = [
        ("application", "chrome"),
        ("application", "chromium"),
        ("application", "Steam"),
    ];

    for (attr_key, attr_val) in candidates {
        if let Ok(out) = std::process::Command::new("secret-tool")
            .args(["lookup", attr_key, attr_val])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                let password = String::from_utf8_lossy(&out.stdout).trim().to_string();
                return Ok(password.into_bytes());
            }
        }
    }

    anyhow::bail!("No keyring password found via secret-tool")
}

#[cfg(target_os = "linux")]
fn decrypt_cookie_linux(encrypted: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Encrypted cookie value too short");
    }

    // Unencrypted plain-text value
    if !encrypted.starts_with(b"v10") && !encrypted.starts_with(b"v11") {
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    let ciphertext = &encrypted[3..];
    let iv = [b' '; 16];
    let password = get_keyring_password().unwrap_or_else(|_| b"peanuts".to_vec());

    // Try AES-128 first, then AES-256, each with the real password and the
    // known fallback passwords used by Chromium-based browsers.
    let passwords: &[&[u8]] = &[password.as_slice(), b"peanuts", b""];

    for pw in passwords {
        let mut key = [0u8; 16];
        pbkdf2_hmac::<Sha1>(pw, b"saltysalt", 1, &mut key);

        type Aes128CbcDec = cbc::Decryptor<Aes128>;
        let mut buf = ciphertext.to_vec();
        if let Ok(decrypted) = Aes128CbcDec::new(&key.into(), &iv.into())
            .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
        {
            return Ok(String::from_utf8_lossy(decrypted).to_string());
        }
    }

    for pw in passwords {
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha1>(pw, b"saltysalt", 1, &mut key);

        type Aes256CbcDec = cbc::Decryptor<Aes256>;
        let mut buf = ciphertext.to_vec();
        if let Ok(decrypted) = Aes256CbcDec::new(&key.into(), &iv.into())
            .decrypt_padded::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
        {
            return Ok(String::from_utf8_lossy(decrypted).to_string());
        }
    }

    anyhow::bail!("All AES decryption attempts failed for cookie")
}

// ---------------------------------------------------------------------------
// Windows: read a file that may be locked by another process
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn read_locked_file_windows(src_path: &Path) -> Result<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = src_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle: HANDLE = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0x8000_0000u32, // GENERIC_READ
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
            .context("CreateFileW failed — cannot open locked cookie database")?
    };

    let mut size = 0u64;
    unsafe {
        windows::Win32::Storage::FileSystem::GetFileSizeEx(
            handle,
            &mut size as *mut u64 as *mut _,
        )
            .ok()
            .context("GetFileSizeEx failed")?;
    }

    let mut buf = vec![0u8; size as usize];
    let mut bytes_read = 0u32;

    unsafe {
        ReadFile(handle, Some(buf.as_mut_slice()), Some(&mut bytes_read), None)
            .ok()
            .context("ReadFile failed")?;
        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }

    buf.truncate(bytes_read as usize);
    Ok(buf)
}

// ---------------------------------------------------------------------------
// DB copy — three-attempt strategy on Windows, direct on Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn copy_db(src_path: &Path, dst_path: &Path) -> Result<()> {
    // Attempt 1: direct SQLite open (works when Steam is closed)
    if let Ok(src) = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        if let Ok(mut dst) = Connection::open(dst_path) {
            if let Ok(backup) = rusqlite::backup::Backup::new(&src, &mut dst) {
                if backup
                    .run_to_completion(100, std::time::Duration::from_millis(5), None)
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
    }

    // Attempt 2: immutable URI (sometimes works with Steam running)
    let forward = src_path
        .to_str()
        .context("Cookie DB path is not valid UTF-8")?
        .replace('\\', "/");
    let uri = format!(
        "file:///{}?immutable=1&mode=ro",
        forward.trim_start_matches('/')
    );

    if let Ok(src) = Connection::open_with_flags(
        &uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_URI
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        if let Ok(mut dst) = Connection::open(dst_path) {
            if let Ok(backup) = rusqlite::backup::Backup::new(&src, &mut dst) {
                if backup
                    .run_to_completion(100, std::time::Duration::from_millis(5), None)
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
    }

    // Attempt 3: raw Win32 read with full share flags, then copy WAL/SHM too
    let db_bytes = read_locked_file_windows(src_path)
        .context("Could not read locked cookie database")?;
    std::fs::write(dst_path, &db_bytes)
        .context("Could not write cookie database to temp path")?;

    for suffix in &["-wal", "-shm"] {
        let src_side = src_path.with_file_name(format!(
            "{}{}",
            src_path.file_name().and_then(|n| n.to_str()).unwrap_or("Cookies"),
            suffix
        ));
        let dst_side = dst_path.with_file_name(format!(
            "{}{}",
            dst_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("swu_cookies_tmp.db"),
            suffix
        ));
        if src_side.exists() {
            if let Ok(bytes) = read_locked_file_windows(&src_side) {
                let _ = std::fs::write(&dst_side, bytes);
            }
        }
    }

    Connection::open_with_flags(dst_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map(|_| ())
        .context("Copied database is not readable by SQLite")
}

#[cfg(target_os = "linux")]
fn copy_db(src_path: &Path, dst_path: &Path) -> Result<()> {
    let src = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
        .context("Could not open Steam cookie database")?;

    let mut dst = Connection::open(dst_path).context("Could not create temp database")?;

    let backup =
        rusqlite::backup::Backup::new(&src, &mut dst).context("Could not initialize backup")?;

    backup
        .run_to_completion(100, std::time::Duration::from_millis(5), None)
        .context("Could not complete database backup")
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn get_steam_cookies() -> Result<SteamCookies> {
    #[cfg(target_os = "windows")]
    let aes_key = get_aes_key().context("Could not get AES decryption key")?;

    let db_path = get_cookie_db_path()?;
    let tmp_path = std::env::temp_dir().join("swu_cookies_tmp.db");

    copy_db(&db_path, &tmp_path)?;

    let conn = Connection::open_with_flags(
        &tmp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
        .context("Could not open copied cookie database")?;

    let mut session_id: Option<String> = None;
    let mut login_secure: Option<String> = None;

    let mut stmt = conn.prepare(
        "SELECT name, value, encrypted_value FROM cookies
         WHERE host_key LIKE '%steamcommunity.com'
         AND name IN ('sessionid', 'steamLoginSecure')",
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
            #[cfg(target_os = "windows")]
            {
                decrypt_cookie_windows(&encrypted, &aes_key).unwrap_or_default()
            }
            #[cfg(target_os = "linux")]
            {
                decrypt_cookie_linux(&encrypted).unwrap_or_default()
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
        session_id: session_id
            .filter(|s| !s.is_empty())
            .context("sessionid cookie not found — are you logged into Steam?")?,
        login_secure: login_secure
            .filter(|s| !s.is_empty())
            .context("steamLoginSecure cookie not found — are you logged into Steam?")?,
    })
}