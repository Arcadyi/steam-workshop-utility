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
fn decrypt_dpapi(data: &[u8]) -> Result<String> {
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

        windows::Win32::Foundation::LocalFree(Some(HLOCAL(output.pbData as _)));

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
    if encrypted.is_empty() {
        anyhow::bail!("Empty encrypted value");
    }

    #[cfg(target_os = "windows")]
    {
        if encrypted.len() >= 3 && (encrypted.starts_with(b"v10") || encrypted.starts_with(b"v11")) {
            return decrypt_dpapi(&encrypted[3..]);
        }
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        return decrypt_cookie_linux(encrypted);
    }
}

fn copy_db(src_path: &PathBuf, dst_path: &PathBuf) -> Result<()> {
    let src = Connection::open_with_flags(
        src_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ).context("Could not open Steam cookie database")?;

    let mut dst = Connection::open(dst_path)
        .context("Could not create temp database")?;

    let backup = rusqlite::backup::Backup::new(&src, &mut dst)
        .context("Could not initialize backup")?;

    backup.run_to_completion(100, std::time::Duration::from_millis(5), None)
        .context("Could not complete database backup")?;

    Ok(())
}

pub fn get_steam_cookies() -> Result<SteamCookies> {
    let log_path = std::env::temp_dir().join("swu_cookies_debug.log");
    let mut log = std::fs::File::create(&log_path).ok();

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
            dlog!("  using plaintext");
            value
        } else if !encrypted.is_empty() {
            dlog!("  attempting decryption...");
            match decrypt_cookie(&encrypted) {
                Ok(v) => { dlog!("  decrypted ok len={}", v.len()); v }
                Err(e) => { dlog!("  decryption failed: {}", e); String::new() }
            }
        } else {
            dlog!("  no value or encrypted_value");
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