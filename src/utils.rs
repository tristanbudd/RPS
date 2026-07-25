use crate::config::PasteConfig;
use rand::{distributions::Alphanumeric, thread_rng, Rng};

/// Generates a unique paste ID based on the format specified in configuration
pub fn generate_id(config: &PasteConfig) -> String {
    match config.id_type.as_str() {
        "numeric" => {
            const CHARSET: &[u8] = b"0123456789";
            let mut rng = thread_rng();
            (0..config.id_length)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
        "lowercase" => {
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = thread_rng();
            (0..config.id_length)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
        "uuid" => uuid::Uuid::new_v4().to_string(),
        _ => thread_rng()
            .sample_iter(&Alphanumeric)
            .take(config.id_length)
            .map(char::from)
            .collect(),
    }
}
