// SPDX-License-Identifier: GPL-2.0-or-later

//! Verification against gpg-made fixtures (`testdata/signing`, test key 76537ED4...).

use super::*;

const DATA: &[u8] = include_bytes!("../../testdata/signing/data.txt");
const SIG512: &[u8] = include_bytes!("../../testdata/signing/data.txt.sha512.sig");
const SIG256: &[u8] = include_bytes!("../../testdata/signing/data.txt.sha256.sig");
const SIG_OTHER: &[u8] = include_bytes!("../../testdata/signing/data.txt.other.sig");
const TEST_KEY: &[u8] = include_bytes!("../../testdata/signing/test-key.asc");
const TEST_FPR: &str = "76537ED4218CABF125D559362E55F95E8F6A3B51";
const TEST_UID: &str = "paku test key <test@paku.invalid>";

fn ring() -> Keyring {
    Keyring::load(TEST_KEY, "").unwrap()
}

#[test]
fn valid_sha512_and_sha256_signatures() {
    let ring = ring();
    for sig in [SIG512, SIG256] {
        let key = verify(&ring, DATA, sig).unwrap();
        assert_eq!(hex(&key.key.fingerprint), TEST_FPR);
    }
}

#[test]
fn tampered_data_is_invalid() {
    let mut data = DATA.to_vec();
    data[0] ^= 1;
    let err = verify(&ring(), data.as_slice(), SIG512).unwrap_err();
    assert_eq!(err, SigError::Invalid(TEST_UID.into()));
    assert_eq!(err.to_string(), format!("signature from \"{TEST_UID}\" is invalid"));
}

#[test]
fn tampered_signature_is_invalid() {
    let mut sig = SIG512.to_vec();
    let last = sig.len() - 1;
    sig[last] ^= 1;
    assert_eq!(
        verify(&ring(), DATA, &sig).unwrap_err(),
        SigError::Invalid(TEST_UID.into())
    );
}

#[test]
fn unknown_key() {
    let err = verify(&ring(), DATA, SIG_OTHER).unwrap_err();
    assert_eq!(
        err,
        SigError::UnknownKey("202947B0DEE8FBD12032584F13E9040537BBCCE6".into())
    );
}

#[test]
fn revoked_key() {
    let ring = Keyring::load(TEST_KEY, TEST_FPR).unwrap();
    assert_eq!(
        verify(&ring, DATA, SIG512).unwrap_err(),
        SigError::Revoked(TEST_UID.into())
    );
}

#[test]
fn malformed_signature() {
    assert_eq!(verify(&ring(), DATA, b"junk").unwrap_err(), SigError::Malformed);
}

#[test]
fn check_levels() {
    let ring = ring();
    let dir = paku_platform::TempDir::new("paku-test-").unwrap();
    let path = dir.path().join("data.txt");
    std::fs::write(&path, DATA).unwrap();

    assert!(check_file(Check::Never, &ring, &path, Some(b"junk")).unwrap().is_none());
    assert!(check_file(Check::Optional, &ring, &path, None).unwrap().is_none());
    assert!(
        check_file(Check::Optional, &ring, &path, Some(SIG512))
            .unwrap()
            .is_some()
    );
    assert!(check_file(Check::Optional, &ring, &path, Some(SIG_OTHER)).is_err());
    assert_eq!(
        check_file(Check::Required, &ring, &path, None).unwrap_err(),
        SigError::Missing
    );
    assert!(
        check_file(Check::Required, &ring, &path, Some(SIG256))
            .unwrap()
            .is_some()
    );
}

/// Live check against MSYS2: `cargo test -p alpm -- --ignored`.
#[test]
#[ignore = "downloads msys.db from repo.msys2.org"]
fn live_msys_db_signature() {
    let dir = paku_platform::TempDir::new("paku-test-").unwrap();
    let servers = vec!["https://repo.msys2.org/msys/x86_64".to_string()];
    let opts = crate::download::Options::default();
    for file in ["msys.db", "msys.db.sig"] {
        crate::download::fetch(&servers, file, &dir.path().join(file), true, &opts).unwrap();
    }
    let ring = Keyring::embedded().unwrap();
    let sig = std::fs::read(dir.path().join("msys.db.sig")).unwrap();
    let key = check_file(Check::Required, &ring, &dir.path().join("msys.db"), Some(&sig)).unwrap();
    assert!(key.is_some());
}
