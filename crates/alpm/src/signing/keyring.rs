// SPDX-License-Identifier: GPL-2.0-or-later

//! A keyring of trusted signing keys, by default MSYS2's (embedded at build time).

use super::SigError;
use super::key::{PublicKey, hex};
use super::packet::{TAG_PUBLIC_KEY, TAG_PUBLIC_SUBKEY, TAG_SIGNATURE, TAG_USER_ID, packets};
use super::signature::Signature;
use crate::signing::armor::dearmor;

const SIG_KEY_REVOCATION: u8 = 0x20;
const SIG_SUBKEY_REVOCATION: u8 = 0x28;

/// `msys2.gpg` and `msys2-revoked` from the `msys2-keyring` package.
const EMBEDDED_KEYS: &[u8] = include_bytes!("../../keyring/msys2.gpg");
const EMBEDDED_REVOKED: &str = include_str!("../../keyring/msys2-revoked");

/// A key or subkey usable for verification.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    pub key: PublicKey,
    /// Fingerprint of the primary key (the key's own for primary keys).
    pub primary: [u8; 20],
    /// First user ID of the primary key, e.g. `Name <mail>`.
    pub uid: Option<String>,
    /// Revoked by the keyring's revocation list or a revocation signature.
    pub revoked: bool,
}

impl KeyEntry {
    /// User ID if known, else the primary key's fingerprint.
    pub fn display_name(&self) -> String {
        self.uid.clone().unwrap_or_else(|| hex(&self.primary))
    }
}

/// Keys trusted for package and database signatures.
///
/// Every key in the keyring counts as trusted unless revoked: the keyring itself is
/// the trust anchor, like pacman's after `pacman-key --populate`.
#[derive(Debug, Clone, Default)]
pub struct Keyring {
    keys: Vec<KeyEntry>,
}

impl Keyring {
    /// MSYS2's keyring, embedded in the binary.
    pub fn embedded() -> Result<Self, SigError> {
        Self::load(EMBEDDED_KEYS, EMBEDDED_REVOKED)
    }

    /// Loads keys (armored or binary) and a revocation list with one fingerprint per line.
    pub fn load(keys: &[u8], revoked: &str) -> Result<Self, SigError> {
        let data = dearmor(keys)?;
        let mut ring = Self::default();
        let mut current: Option<usize> = None;
        for p in packets(&data)? {
            match p.tag {
                TAG_PUBLIC_KEY => current = ring.add(p.body, None),
                TAG_PUBLIC_SUBKEY => ring.add_subkey(p.body, &mut current),
                TAG_USER_ID => ring.set_uid(current, p.body),
                TAG_SIGNATURE => ring.apply_revocation(current, p.body),
                _ => {}
            }
        }
        ring.apply_revoked_list(revoked);
        Ok(ring)
    }

    /// Adds a key; unsupported keys (e.g. v3) are skipped. Returns its index.
    fn add(&mut self, body: &[u8], primary: Option<&KeyEntry>) -> Option<usize> {
        let key = PublicKey::parse(body).ok()?;
        let entry = KeyEntry {
            primary: primary.map_or(key.fingerprint, |p| p.primary),
            uid: primary.and_then(|p| p.uid.clone()),
            revoked: primary.is_some_and(|p| p.revoked),
            key,
        };
        self.keys.push(entry);
        Some(self.keys.len() - 1)
    }

    /// Subkeys belong to the most recent primary key; `current` then points at the subkey.
    fn add_subkey(&mut self, body: &[u8], current: &mut Option<usize>) {
        let Some(i) = *current else { return };
        let primary_fpr = self.keys[i].primary;
        let Some(primary) = self.keys.iter().find(|k| k.key.fingerprint == primary_fpr).cloned() else {
            return;
        };
        *current = self.add(body, Some(&primary)).or(Some(i));
    }

    /// Records the first user ID of a primary key and copies it to its subkeys.
    fn set_uid(&mut self, current: Option<usize>, body: &[u8]) {
        let Some(i) = current else { return };
        let primary = self.keys[i].primary;
        let uid = String::from_utf8_lossy(body).into_owned();
        for k in self.keys.iter_mut().filter(|k| k.primary == primary && k.uid.is_none()) {
            k.uid = Some(uid.clone());
        }
    }

    /// A revocation signature in the keyring revokes the key it follows.
    fn apply_revocation(&mut self, current: Option<usize>, body: &[u8]) {
        let Some(i) = current else { return };
        let Ok(sig) = Signature::parse(body) else { return };
        let is_primary = self.keys[i].key.fingerprint == self.keys[i].primary;
        match sig.sig_type {
            SIG_KEY_REVOCATION if is_primary => self.revoke_primary(self.keys[i].primary),
            SIG_SUBKEY_REVOCATION if !is_primary => self.keys[i].revoked = true,
            _ => {}
        }
    }

    fn apply_revoked_list(&mut self, revoked: &str) {
        for line in revoked.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let Some(fpr) = self
                .keys
                .iter()
                .find(|k| hex(&k.key.fingerprint).eq_ignore_ascii_case(line))
            else {
                continue;
            };
            let primary = fpr.primary;
            self.revoke_primary(primary);
        }
    }

    fn revoke_primary(&mut self, primary: [u8; 20]) {
        for k in self.keys.iter_mut().filter(|k| k.primary == primary) {
            k.revoked = true;
        }
    }

    /// Finds the key that made a signature, by fingerprint or else by key ID.
    pub fn find(&self, fingerprint: Option<[u8; 20]>, key_id: Option<[u8; 8]>) -> Option<&KeyEntry> {
        if let Some(fpr) = fingerprint {
            return self.keys.iter().find(|k| k.key.fingerprint == fpr);
        }
        let id = key_id?;
        self.keys.iter().find(|k| k.key.key_id() == id)
    }

    /// Number of keys and subkeys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the keyring has no keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_fpr(ring: &Keyring, fpr: &str) -> KeyEntry {
        ring.keys
            .iter()
            .find(|k| hex(&k.key.fingerprint) == fpr)
            .cloned()
            .unwrap()
    }

    #[test]
    fn embedded_msys2_keyring() {
        let ring = Keyring::embedded().unwrap();
        assert!(ring.len() >= 20, "{} keys", ring.len());
        // current packager key for databases and packages
        let packager = by_fpr(&ring, "5F944B027F7FE2091985AA2EFA11531AA0AA7F57");
        assert!(!packager.revoked);
        assert!(packager.key.rsa.is_some());
        assert!(packager.uid.is_some());
        // listed in msys2-revoked, as is its encryption subkey
        assert!(by_fpr(&ring, "123D4D51A1793859C2BE916BBBE514E53E0D0813").revoked);
        assert!(by_fpr(&ring, "8EFDA796E76FCC43B08522EA5E84D3268603AA9D").revoked);
    }

    #[test]
    fn subkeys_inherit_primary_uid() {
        let ring = Keyring::embedded().unwrap();
        let sub = by_fpr(&ring, "4A6129F4E4B84AE46ED7F635628F528CF3053E04");
        assert_eq!(hex(&sub.primary), "87771331B3F1FF5263856A6D974C8BE49078F532");
        assert_eq!(sub.uid, by_fpr(&ring, "87771331B3F1FF5263856A6D974C8BE49078F532").uid);
    }

    #[test]
    fn lookup_by_key_id() {
        let ring = Keyring::load(include_bytes!("../../testdata/signing/test-key.asc"), "").unwrap();
        let id = [0x2E, 0x55, 0xF9, 0x5E, 0x8F, 0x6A, 0x3B, 0x51];
        let k = ring.find(None, Some(id)).unwrap();
        assert_eq!(k.uid.as_deref(), Some("paku test key <test@paku.invalid>"));
        assert!(ring.find(None, Some([0; 8])).is_none());
    }
}
