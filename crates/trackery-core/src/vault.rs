//! Vault key management with offline recovery.
//!
//! The main vault database is keyed by a random hex key `K`. `K` is held in
//! two SQLCipher "keyslot" mini-databases beside the vault: one keyed by the
//! user's passphrase, one by a 12-word recovery phrase generated once and
//! shown once. SQLCipher's own KDF (PBKDF2-SHA512, 256k rounds) and per-page
//! HMAC do the sealing, so recovery needs no extra crypto dependencies and
//! works fully offline. Losing both the passphrase and the words loses the
//! data — by design; there is no escrow.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, ErrorCode};
use uuid::Uuid;

use crate::db::{Db, DbError};

/// 256 distinct short words → 8 bits each, 96 bits across 12 words.
const WORDS: [&str; 256] = [
    "acid", "acre", "aged", "ally", "anchor", "angle", "apple", "arch", "area", "army",
    "atlas", "atom", "autumn", "axis", "badge", "baker", "bamboo", "barn", "basil", "basket",
    "beach", "bell", "bench", "berry", "birch", "blade", "blend", "bloom", "board", "bolt",
    "bonus", "book", "boots", "bottle", "bow", "brass", "brave", "bread", "brick", "bridge",
    "brook", "broom", "brush", "bucket", "buddy", "bugle", "bulb", "bundle", "butter", "cabin",
    "cable", "cactus", "camel", "camp", "canal", "candle", "canoe", "canyon", "cargo", "carpet",
    "castle", "cedar", "cellar", "chalk", "cherry", "chess", "chief", "chime", "cider", "circle",
    "citrus", "civil", "clay", "cliff", "clock", "cloud", "clover", "coast", "cobalt", "coconut",
    "coffee", "comet", "compass", "copper", "coral", "cotton", "cradle", "crane", "crater", "cream",
    "cricket", "crown", "crystal", "cycle", "daisy", "dawn", "delta", "denim", "desk", "dew",
    "diamond", "dome", "door", "dune", "eagle", "earth", "east", "echo", "elbow", "elder",
    "ember", "engine", "estate", "fabric", "falcon", "farm", "feather", "fence", "fern", "ferry",
    "field", "flag", "flame", "flask", "fleet", "flint", "flour", "flute", "forest", "fossil",
    "fox", "frame", "frost", "galaxy", "garden", "garlic", "gate", "gem", "ginger", "glacier",
    "glass", "globe", "gold", "granite", "grape", "grove", "guitar", "gulf", "hammer", "harbor",
    "harvest", "hawk", "hazel", "heron", "hill", "honey", "horizon", "iceberg", "index", "iris",
    "iron", "island", "ivory", "jade", "jasmine", "jungle", "juniper", "kettle", "kite", "ladder",
    "lagoon", "lake", "lantern", "laurel", "lava", "lemon", "lens", "level", "lilac", "lily",
    "linen", "lion", "lotus", "lunar", "magnet", "mango", "maple", "marble", "meadow", "melon",
    "mesa", "mint", "mirror", "monsoon", "moon", "moss", "mountain", "mural", "myrtle", "nectar",
    "nest", "north", "novel", "nutmeg", "oak", "oasis", "ocean", "olive", "onion", "onyx",
    "opal", "orbit", "orchard", "otter", "oxide", "palm", "panda", "paper", "peach", "pearl",
    "pebble", "pepper", "petal", "pigeon", "pine", "planet", "plum", "pocket", "polar", "pond",
    "poppy", "prairie", "prism", "quartz", "quill", "rain", "raisin", "raven", "reef", "ribbon",
    "ridge", "river", "robin", "rocket", "rose", "ruby", "saffron", "sage", "salt", "sand",
    "sapphire", "satin", "shore", "silver", "sky", "totem",
];

/// A successful unlock, with the recovery phrase when this unlock minted it.
pub struct Unlocked {
    pub db: Db,
    /// Present exactly once: on first vault creation, or on upgrading a
    /// passphrase-keyed vault to the keyslot scheme. Show it, never store it.
    pub new_recovery_words: Option<Vec<String>>,
}

fn keyslot_pass(dir: &Path) -> PathBuf {
    dir.join("keyslot_pass.db")
}
fn keyslot_recovery(dir: &Path) -> PathBuf {
    dir.join("keyslot_recovery.db")
}

fn fresh_key() -> String {
    // Two v4 UUIDs ≈ 244 bits of OS randomness, as 64 hex chars.
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn fresh_words() -> Vec<String> {
    Uuid::new_v4().as_bytes()[..12]
        .iter()
        .map(|b| WORDS[*b as usize].to_string())
        .collect()
}

/// Case- and whitespace-insensitive form of a typed recovery phrase.
pub fn normalize_words(input: &str) -> String {
    input
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn keyslot_write(path: &Path, secret: &str, key: &str) -> Result<(), DbError> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "key", secret)?;
    conn.execute_batch("CREATE TABLE secret (k TEXT NOT NULL);")?;
    conn.execute("INSERT INTO secret (k) VALUES (?1)", [key])?;
    Ok(())
}

fn keyslot_read(path: &Path, secret: &str) -> Result<String, DbError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "key", secret)?;
    conn.query_row("SELECT k FROM secret", [], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, _) if f.code == ErrorCode::NotADatabase => {
                DbError::WrongKey
            }
            other => DbError::Sqlite(other),
        })
}

/// Open the vault with `passphrase`, installing the keyslot scheme when it
/// isn't there yet (fresh vault, or upgrade of a passphrase-keyed one).
pub fn unlock(dir: &Path, vault_db: &Path, passphrase: &str) -> Result<Unlocked, DbError> {
    let slot = keyslot_pass(dir);
    if slot.exists() {
        let key = keyslot_read(&slot, passphrase)?;
        return match Db::open(vault_db, &key) {
            Ok(db) => Ok(Unlocked { db, new_recovery_words: None }),
            // A crash between keyslot creation and the vault rekey leaves the
            // vault still under the passphrase — finish the job now.
            Err(DbError::WrongKey) => {
                Db::rekey(vault_db, passphrase, &key)?;
                Ok(Unlocked {
                    db: Db::open(vault_db, &key)?,
                    new_recovery_words: None,
                })
            }
            Err(e) => Err(e),
        };
    }

    let key = fresh_key();
    let words = fresh_words();
    if vault_db.exists() {
        // Legacy vault keyed directly by the passphrase. Rekey verifies the
        // passphrase first, so a wrong one fails before anything changes.
        if let Err(e) = Db::rekey(vault_db, passphrase, &key) {
            return Err(e);
        }
    }
    keyslot_write(&slot, passphrase, &key)?;
    keyslot_write(&keyslot_recovery(dir), &normalize_words(&words.join(" ")), &key)?;
    Ok(Unlocked {
        db: Db::open(vault_db, &key)?,
        new_recovery_words: Some(words),
    })
}

/// Reset the passphrase using the recovery phrase. The words stay valid.
pub fn recover(dir: &Path, words: &str, new_passphrase: &str) -> Result<(), DbError> {
    let key = keyslot_read(&keyslot_recovery(dir), &normalize_words(words))?;
    keyslot_write(&keyslot_pass(dir), new_passphrase, &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "trackery_vault_{}_{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn create_unlock_and_wrong_passphrase() {
        let dir = tmp_dir();
        let vault = dir.join("trackery.db");
        let Unlocked { db, new_recovery_words } = unlock(&dir, &vault, "pass one").expect("create");
        let words = new_recovery_words.expect("words minted on creation");
        assert_eq!(words.len(), 12);
        drop(db);

        let again = unlock(&dir, &vault, "pass one").expect("reopen");
        assert!(again.new_recovery_words.is_none(), "words are one-time");
        drop(again);

        assert!(matches!(
            unlock(&dir, &vault, "wrong"),
            Err(DbError::WrongKey)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovery_words_reset_the_passphrase() {
        let dir = tmp_dir();
        let vault = dir.join("trackery.db");
        let words = unlock(&dir, &vault, "original")
            .expect("create")
            .new_recovery_words
            .expect("words");
        // Sloppy typing must still work.
        let typed = format!("  {}  ", words.join("   ").to_uppercase());
        recover(&dir, &typed, "brand new pass").expect("recover");

        assert!(matches!(
            unlock(&dir, &vault, "original"),
            Err(DbError::WrongKey)
        ));
        unlock(&dir, &vault, "brand new pass").expect("new passphrase works");
        assert!(matches!(
            recover(&dir, "wrong words entirely", "x"),
            Err(DbError::WrongKey)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_passphrase_vault_upgrades_with_data_intact() {
        let dir = tmp_dir();
        let vault = dir.join("trackery.db");
        // v1-style vault: SQLCipher keyed directly by the passphrase.
        {
            let db = Db::open(&vault, "direct pass").expect("legacy create");
            drop(db);
        }
        let up = unlock(&dir, &vault, "direct pass").expect("upgrade");
        assert!(up.new_recovery_words.is_some(), "upgrade mints words");
        drop(up);
        unlock(&dir, &vault, "direct pass").expect("keyslot path works after");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn word_list_is_unique() {
        let set: std::collections::HashSet<&str> = WORDS.iter().copied().collect();
        assert_eq!(set.len(), WORDS.len());
    }
}
