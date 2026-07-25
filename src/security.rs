use magic_crypt::MagicCryptTrait;

/// Hashes a password using bcrypt.
pub fn hash_password(password: &str) -> String {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("Error | Hashing password failed")
}

/// Verifies a password against a hash using bcrypt.
pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

/// Encrypts content via AES-256-CBC using magic-crypt.
pub fn encrypt_content(content: &str, key: &str) -> String {
    let mc = magic_crypt::new_magic_crypt!(key, 256);
    mc.encrypt_str_to_base64(content)
}

/// Decrypts content via AES-256-CBC using magic-crypt, falling back to original content if decryption fails.
pub fn decrypt_content(content: &str, key: &str) -> String {
    let mc = magic_crypt::new_magic_crypt!(key, 256);
    match mc.decrypt_base64_to_string(content) {
        Ok(plain) => plain,
        Err(e) => {
            eprintln!("Error | Decryption failure: {:?}", e);
            content.to_string()
        }
    }
}
