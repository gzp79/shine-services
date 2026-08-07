use rustls::RootCertStore;
use rustls_native_certs::load_native_certs;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
#[error("Failed to load any usable native certs")]
pub struct CertError;

pub fn get_root_cert_store() -> Result<RootCertStore, CertError> {
    let mut store = RootCertStore::empty();
    let certs_result = load_native_certs();

    // `load_native_certs` can report per-cert errors (e.g. a single unparsable/expired system
    // cert) while still returning a fully usable set of certs. Those errors are not fatal on their
    // own: only fail if we could not add a single usable cert to the store. Otherwise a lone bad
    // cert in the OS trust store would refuse service startup despite a working trust anchor set.
    if !certs_result.errors.is_empty() {
        log::warn!(
            "Some native certs failed to load (continuing with the usable ones): {:?}",
            certs_result.errors
        );
    }

    let (added, ignored) = store.add_parsable_certificates(certs_result.certs);
    log::debug!("Loaded {added} native root certs ({ignored} ignored as unparsable)");

    if store.is_empty() {
        Err(CertError)
    } else {
        Ok(store)
    }
}
