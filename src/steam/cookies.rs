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

#[derive(Debug, Clone)]
pub struct SteamCookies {
    pub session_id: String,
    pub login_secure: String,
}

#[cfg(target_os = "windows")]
fn get_cookie_db_path() -> Result<PathBuf> {
    use std::io::Write;
    let log_path = std::env::temp_dir().join("swu_cookies_debug.log");
    let mut log = std::fs::OpenOptions::new()
        .create(true).append(true).open(&log_path).ok();

    macro_rules! dlog {
        ($($arg:tt)*) => {
            if let Some(ref mut f) = log {
                let _ = writeln!(f, $($arg)*);
            }
        }
    }

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

    for path in &candidates {
        dlog!("Trying: {}", path.display());
        if path.exists() {
            dlog!("Found at: {}", path.display());
            return Ok(path.clone());
        }
    }

    anyhow::bail!("Steam cookie database not found — checked {} locations", candidates.len())
}

#[cfg(target_os = "linux")]
fn get_cookie_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let base = home.join(".local/share/Steam/config/htmlcache/Default");

    let candidates = [
        base.join("Network/Cookies"),
        base.join("Cookies"),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    anyhow::bail!(
        "Steam cookie database not found — checked {}",
        candidates.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(target_os = "windows")]
fn get_aes_key() -> Result<Vec<u8>> {
    use std::io::Write;
    let log_path = std::env::temp_dir().join("swu_cookies_debug.log");
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path).ok();
    use base64::Engine;

    macro_rules! dlog {
        ($($arg:tt)*) => {
            if let Some(ref mut f) = log { let _ = writeln!(f, $($arg)*); }
        }
    }

    // Steam's Local State is next to htmlcache
    let base = dirs::data_local_dir().context("No local data dir")?;
    let local_state_path = base.join("Steam\\htmlcache\\Local State");
    dlog!("Local State path: {}", local_state_path.display());

    let contents = std::fs::read_to_string(&local_state_path)
        .context("Could not read Local State file")?;

    // Parse JSON to get os_crypt.encrypted_key
    let json: serde_json::Value = serde_json::from_str(&contents)
        .context("Could not parse Local State JSON")?;

    let encrypted_key_b64 = json["os_crypt"]["encrypted_key"]
        .as_str()
        .context("No os_crypt.encrypted_key in Local State")?;

    dlog!("encrypted_key_b64 len: {}", encrypted_key_b64.len());

    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .context("Could not base64-decode encrypted_key")?;

    dlog!("encrypted_key bytes len: {}", encrypted_key.len());

    // First 5 bytes are "DPAPI" prefix, skip them
    if encrypted_key.len() < 5 || &encrypted_key[..5] != b"DPAPI" {
        anyhow::bail!("encrypted_key does not start with DPAPI prefix");
    }

    let dpapi_blob = &encrypted_key[5..];
    dlog!("dpapi_blob len: {}", dpapi_blob.len());

    // Decrypt with DPAPI to get the raw AES key
    let aes_key = decrypt_dpapi(dpapi_blob)
        .context("DPAPI decryption of AES key failed")?;
    // aes_key is already Vec<u8>, no .into_bytes() needed

    dlog!("AES key len: {}", aes_key.len()); // will now print 32
    Ok(aes_key)  // ← not aes_key.into_bytes()
}

#[cfg(target_os = "windows")]
fn decrypt_dpapi(data: &[u8]) -> Result<Vec<u8>> {  // ← Vec<u8>, not String
    use windows::Win32::Foundation::HLOCAL;
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &mut input,
            None, None, None, None, 0,
            &mut output,
        ).ok().context("CryptUnprotectData failed")?;

        let bytes = std::slice::from_raw_parts(
            output.pbData,
            output.cbData as usize,
        ).to_vec();

        windows::Win32::Foundation::LocalFree(Some(HLOCAL(output.pbData as _)));

        Ok(bytes)  // ← return raw bytes directly
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

#[cfg(target_os = "windows")]
fn decrypt_cookie_windows(encrypted: &[u8], aes_key: &[u8]) -> Result<String> {
    if encrypted.len() < 3 {
        anyhow::bail!("Too short");
    }

    let prefix = &encrypted[..3];

    if prefix != b"v10" && prefix != b"v11" {
        let bytes = decrypt_dpapi(encrypted).context("Legacy DPAPI decrypt failed")?;
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    let rest = &encrypted[3..];
    if rest.len() < 12 + 16 {
        anyhow::bail!("Encrypted value too short for AES-GCM");
    }

    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};

    let key: &aes_gcm::aead::Key<Aes256Gcm> = aes_key.try_into()
        .map_err(|_| anyhow::anyhow!("AES key must be 32 bytes, got {}", aes_key.len()))?;
    let cipher = Aes256Gcm::new(key);

    let nonce: &aes_gcm::Nonce<_> = (&rest[..12]).try_into()
        .map_err(|_| anyhow::anyhow!("Nonce must be 12 bytes"))?;
    let ciphertext_and_tag = &rest[12..];

    let plaintext = cipher.decrypt(nonce, ciphertext_and_tag)
        .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed: {:?}", e))?;

    Ok(String::from_utf8_lossy(&plaintext).to_string())
}

fn decrypt_cookie(encrypted: &[u8]) -> Result<String> {
    if encrypted.is_empty() {
        anyhow::bail!("Empty encrypted value");
    }

    #[cfg(target_os = "windows")]
    {
        if encrypted.len() >= 3 && (encrypted.starts_with(b"v10") || encrypted.starts_with(b"v11")) {
            // This path shouldn't normally be hit — decrypt_cookie_windows handles v10/v11
            // Legacy DPAPI cookies have no prefix
        }
        // Legacy DPAPI-encrypted cookies (no v10/v11 prefix)
        let bytes = decrypt_dpapi(encrypted).context("Legacy DPAPI decrypt failed")?;
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        return decrypt_cookie_linux(encrypted);
    }
}


#[cfg(target_os = "windows")]
fn read_locked_file_windows(src_path: &PathBuf) -> Result<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,  // ← remove GENERIC_READ here
    };


    let wide: Vec<u16> = src_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle: HANDLE = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0x8000_0000u32,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
            .context("CreateFileW failed — cannot open locked cookie database")?
    };

    // Get file size
    let mut size = 0u64;
    unsafe {
        windows::Win32::Storage::FileSystem::GetFileSizeEx(handle, &mut size as *mut u64 as *mut _)
            .ok()
            .context("GetFileSizeEx failed")?;
    }

    let mut buf = vec![0u8; size as usize];
    let mut bytes_read = 0u32;

    unsafe {
        ReadFile(
            handle,
            Some(buf.as_mut_slice()),
            Some(&mut bytes_read),
            None,
        )
            .ok()
            .context("ReadFile failed")?;

        let _ = windows::Win32::Foundation::CloseHandle(handle);
    }

    buf.truncate(bytes_read as usize);
    Ok(buf)
}

#[cfg(target_os = "windows")]
fn copy_db_windows(src_path: &PathBuf, dst_path: &PathBuf) -> Result<()> {
    // Attempt 1: direct open (works when Steam is closed)
    if let Ok(src) = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
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

    // Attempt 2: immutable URI
    let forward = src_path
        .to_str()
        .context("Invalid path")?
        .replace('\\', "/");
    let uri = format!("file:///{}?immutable=1&mode=ro", forward.trim_start_matches('/'));

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

    // Attempt 3: read the locked file via CreateFileW with full share flags,
    // write bytes to temp location, then also grab WAL/SHM the same way.
    let db_bytes = read_locked_file_windows(src_path)
        .context("Could not read locked cookie database")?;
    std::fs::write(dst_path, &db_bytes)
        .context("Could not write cookie database to temp path")?;

    // Pull WAL and SHM files the same way
    for suffix in &["-wal", "-shm"] {
        let src_side = {
            let mut p = src_path.clone();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Cookies")
                .to_string();
            p.set_file_name(format!("{}{}", name, suffix));
            p
        };
        let dst_side = {
            let mut p = dst_path.clone();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("swu_cookies_tmp.db")
                .to_string();
            p.set_file_name(format!("{}{}", name, suffix));
            p
        };
        if src_side.exists() {
            if let Ok(bytes) = read_locked_file_windows(&src_side) {
                let _ = std::fs::write(&dst_side, bytes);
            }
        }
    }

    // Open the written DB to verify SQLite can read it
    Connection::open_with_flags(
        dst_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
        .map(|_| ())
        .context("Copied database is not readable by SQLite")
}

fn copy_db(src_path: &PathBuf, dst_path: &PathBuf) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        return copy_db_windows(src_path, dst_path);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let src = Connection::open_with_flags(
            src_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ).context("Could not open Steam cookie database")?;

        let mut dst = Connection::open(dst_path)
            .context("Could not create temp database")?;

        let backup = rusqlite::backup::Backup::new(&src, &mut dst)
            .context("Could not initialize backup")?;

        backup.run_to_completion(100, std::time::Duration::from_millis(5), None)
            .context("Could not complete database backup")?;

        Ok(())
    }
}


pub fn get_steam_cookies() -> Result<SteamCookies> {
    let log_path = std::env::temp_dir().join("swu_cookies_debug.log");
    let mut log = std::fs::File::create(&log_path).ok();


    #[cfg(target_os = "windows")]
    let aes_key = get_aes_key().context("Could not get AES decryption key")?;

    macro_rules! dlog {
        ($($arg:tt)*) => {
            if let Some(ref mut f) = log {
                use std::io::Write;
                let _ = writeln!(f, $($arg)*);
            }
        }
    }

    dlog!("Starting cookie extraction");

    let db_path = match get_cookie_db_path() {
        Ok(p) => { dlog!("DB path: {}", p.display()); p }
        Err(e) => { dlog!("Failed to get DB path: {}", e); return Err(e); }
    };

    let tmp_path = std::env::temp_dir().join("swu_cookies_tmp.db");
    dlog!("Temp path: {}", tmp_path.display());

    match copy_db(&db_path, &tmp_path) {
        Ok(_) => dlog!("DB copied ok"),
        Err(e) => { dlog!("Copy failed: {}", e); return Err(e); }
    }

    let conn = match Connection::open_with_flags(
        &tmp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => { dlog!("DB opened ok"); c }
        Err(e) => { dlog!("DB open failed: {}", e); return Err(e.into()); }
    };

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

    let mut row_count = 0;
    for row in rows {
        row_count += 1;
        let (name, value, encrypted) = row?;
        dlog!("Row {}: name={} value_len={} encrypted_len={}", row_count, name, value.len(), encrypted.len());

        if !encrypted.is_empty() {
            dlog!("  prefix bytes: {:?}", &encrypted[..encrypted.len().min(4)]);
        }

        let resolved = if !value.is_empty() {
            value
        } else if !encrypted.is_empty() {
            match {
                #[cfg(target_os = "windows")]
                { decrypt_cookie_windows(&encrypted, &aes_key) }
                #[cfg(not(target_os = "windows"))]
                { decrypt_cookie(&encrypted) }
            } {
                Ok(v) => v,
                Err(e) => { dlog!("  decryption failed: {}", e); String::new() }
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

    dlog!("Total rows: {}", row_count);
    dlog!("session_id present: {}", session_id.is_some());
    dlog!("login_secure present: {}", login_secure.is_some());

    let _ = std::fs::remove_file(&tmp_path);

    Ok(SteamCookies {
        session_id: session_id.context("sessionid cookie not found — are you logged into Steam?")?,
        login_secure: login_secure.context("steamLoginSecure cookie not found — are you logged into Steam?")?,
    })
}